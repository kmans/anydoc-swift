//! XLSX chart relationships and data references. No formulas or external links execute.
use crate::error::ConvertError;
use crate::model::{Chart, ChartSeries, Diagnostic, SourceLocation};
use crate::package::relationships::{read_rels, rels_part_for};
use crate::package::xml::{Element, ns};
use crate::package::{Package, path};
use std::collections::HashMap;

pub(super) type WorkbookCells = HashMap<String, HashMap<(u32, u32), (Option<String>, String)>>;

pub(super) fn coordinate(value: &str) -> Option<(u32, u32)> {
    let value = value.replace('$', "");
    let split = value.find(|c: char| c.is_ascii_digit())?;
    let mut col = 0u32;
    for c in value[..split].bytes() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        col = col
            .checked_mul(26)?
            .checked_add(u32::from(c.to_ascii_uppercase() - b'A' + 1))?;
    }
    let row = value[split..].parse::<u32>().ok()?.checked_sub(1)?;
    let col = col.checked_sub(1)?;
    if row >= 1_048_576 || col >= 16_384 {
        return None;
    }
    Some((row, col))
}

fn range(formula: &str, cells: &WorkbookCells) -> Option<Vec<(Option<String>, String)>> {
    let (sheet, refs) = formula.rsplit_once('!')?;
    let sheet = sheet
        .trim_start_matches('=')
        .trim_matches('\'')
        .replace("''", "'");
    if sheet.contains('[') || refs.contains(',') {
        return None;
    }
    let sheet = cells.get(&sheet)?;
    let (a, b) = refs.split_once(':').unwrap_or((refs, refs));
    let (r1, c1) = coordinate(a)?;
    let (r2, c2) = coordinate(b)?;
    if r2 < r1
        || c2 < c1
        || (r2 - r1 + 1).checked_mul(c2 - c1 + 1)? > crate::package::limits::MAX_GRID_SLOTS as u32
    {
        return None;
    }
    let mut out = Vec::new();
    for r in r1..=r2 {
        for c in c1..=c2 {
            out.push(sheet.get(&(r, c)).cloned().unwrap_or_default());
        }
    }
    Some(out)
}

fn cached(
    parent: &Element,
    numeric: bool,
    budget: &mut u64,
) -> Result<Vec<(Option<String>, String)>, ConvertError> {
    let container = parent
        .first_descendant(ns::CHART, if numeric { "numCache" } else { "strCache" })
        .or_else(|| parent.first_descendant(ns::CHART, if numeric { "numLit" } else { "strLit" }))
        .or_else(|| {
            if numeric {
                None
            } else {
                parent
                    .first_descendant(ns::CHART, "numCache")
                    .or_else(|| parent.first_descendant(ns::CHART, "numLit"))
            }
        });
    let Some(container) = container else {
        return Ok(Vec::new());
    };
    let count = container
        .find(ns::CHART, "ptCount")
        .and_then(|e| e.attr_unqualified("val"))
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let points: Vec<_> = container.find_all(ns::CHART, "pt").collect();
    let extent = points
        .iter()
        .filter_map(|e| {
            e.attr_unqualified("idx")?
                .parse::<usize>()
                .ok()?
                .checked_add(1)
        })
        .max()
        .unwrap_or(0)
        .max(count);
    if extent > crate::package::limits::MAX_GRID_SLOTS as usize {
        return Err(ConvertError::ResourceLimit {
            limit: "max_grid_slots",
            detail: "chart cache exceeds point limit".into(),
        });
    }
    charge(extent, budget)?;
    let mut out = vec![(None, String::new()); extent];
    for point in points {
        if let Some(index) = point
            .attr_unqualified("idx")
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|i| *i < extent)
        {
            let text = point
                .find(ns::CHART, "v")
                .map(Element::text)
                .unwrap_or_default();
            let number = if numeric && !text.is_empty() {
                Some(text.clone())
            } else {
                None
            };
            out[index] = (number, text);
        }
    }
    Ok(out)
}

fn values(
    parent: Option<&Element>,
    numeric: bool,
    cells: &WorkbookCells,
    location: &SourceLocation,
    diagnostics: &mut Vec<Diagnostic>,
    budget: &mut u64,
) -> Result<(Vec<(Option<String>, String)>, Vec<String>), ConvertError> {
    let Some(parent) = parent else {
        return Ok((Vec::new(), Vec::new()));
    };
    let formula = parent.first_descendant(ns::CHART, "f").map(Element::text);
    if let Some(resolved) = formula.as_deref().and_then(|f| range(f, cells)) {
        charge(resolved.len(), budget)?;
        // A readable worksheet is authoritative, including its blank points.
        // A stale chart cache must never turn a missing source value into data.
        return Ok((resolved, formula.into_iter().collect()));
    }
    let cache = cached(parent, numeric, budget)?;
    if formula.is_some() && !cache.is_empty() {
        diagnostics.push(Diagnostic {
            code: "chart.cachedData".into(),
            severity: "information".into(),
            location: Some(location.clone()),
            arguments: vec![],
        });
    }
    Ok((cache, formula.into_iter().collect()))
}

pub(super) fn read_charts(
    pkg: &mut Package,
    worksheet: &Element,
    part: &str,
    sheet: &str,
    cells: &WorkbookCells,
    diagnostics: &mut Vec<Diagnostic>,
    budget: &mut u64,
) -> Result<Vec<Chart>, ConvertError> {
    let rels = read_rels(pkg, &rels_part_for(part))?;
    let mut out = Vec::new();
    for drawing in worksheet.find_all(ns::SML, "drawing") {
        let target = drawing
            .attr_qualified(ns::R, "id")
            .and_then(|id| rels.internal_target(id))
            .and_then(|t| path::resolve(part, t).ok());
        let Some(target) = target else {
            diagnostics.push(missing(part, sheet));
            continue;
        };
        let Some(root) = pkg.optional_xml_part(&target.path)? else {
            diagnostics.push(missing(&target.path, sheet));
            continue;
        };
        let drawing_rels = read_rels(pkg, &rels_part_for(&target.path))?;
        for reference in root.descendants(ns::CHART, "chart") {
            let chart_part = reference
                .attr_qualified(ns::R, "id")
                .and_then(|id| drawing_rels.internal_target(id))
                .and_then(|t| path::resolve(&target.path, t).ok());
            let Some(chart_part) = chart_part else {
                diagnostics.push(missing(&target.path, sheet));
                continue;
            };
            let Some(root) = pkg.optional_xml_part(&chart_part.path)? else {
                diagnostics.push(missing(&chart_part.path, sheet));
                continue;
            };
            out.push(parse_chart(
                &root,
                &chart_part.path,
                sheet,
                cells,
                diagnostics,
                budget,
            )?);
        }
    }
    Ok(out)
}
fn missing(part: &str, sheet: &str) -> Diagnostic {
    Diagnostic {
        code: "chart.relationshipUnavailable".into(),
        severity: "warning".into(),
        location: Some(SourceLocation {
            part: part.into(),
            sheet: Some(sheet.into()),
            object: None,
        }),
        arguments: vec![],
    }
}

fn parse_chart(
    root: &Element,
    part: &str,
    sheet: &str,
    cells: &WorkbookCells,
    diagnostics: &mut Vec<Diagnostic>,
    budget: &mut u64,
) -> Result<Chart, ConvertError> {
    let chart = root.first_descendant(ns::CHART, "chart").unwrap_or(root);
    let title = chart
        .find(ns::CHART, "title")
        .map(|e| {
            e.descendants(ns::A, "t")
                .map(Element::text)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let location = SourceLocation {
        part: part.into(),
        sheet: Some(sheet.into()),
        object: Some(if title.is_empty() {
            part.into()
        } else {
            title.clone()
        }),
    };
    let mut result = Chart {
        title,
        location: location.clone(),
        ..Default::default()
    };
    let Some(plot) = chart.find(ns::CHART, "plotArea") else {
        return Ok(result);
    };
    let plots: Vec<_> = plot
        .child_elems()
        .filter(|e| e.ns.as_deref() == Some(ns::CHART) && e.local.ends_with("Chart"))
        .collect();
    result.incompatible_axes = plots.len() != 1;
    for source in plots {
        result.kind = source.local.clone();
        result.grouping = source
            .find(ns::CHART, "grouping")
            .and_then(|e| e.attr_unqualified("val"))
            .unwrap_or("")
            .into();
        result.horizontal = source
            .find(ns::CHART, "barDir")
            .and_then(|e| e.attr_unqualified("val"))
            == Some("bar");
        for series in source.find_all(ns::CHART, "ser") {
            if ["errBars", "trendline"]
                .iter()
                .any(|name| series.find(ns::CHART, name).is_some())
            {
                result.incompatible_axes = true;
                diagnostics.push(Diagnostic {
                    code: "chart.unsupportedDetails".into(),
                    severity: "warning".into(),
                    location: Some(location.clone()),
                    arguments: vec!["error bars or trend lines".into()],
                });
            }

            let (name, _) = values(
                series.find(ns::CHART, "tx"),
                false,
                cells,
                &location,
                diagnostics,
                budget,
            )?;
            let name = name
                .first()
                .map(|(_, s)| s.clone())
                .or_else(|| {
                    series
                        .find(ns::CHART, "tx")
                        .and_then(|e| e.find(ns::CHART, "v"))
                        .map(Element::text)
                })
                .unwrap_or_default();
            let (cat, mut refs) = values(
                series
                    .find(ns::CHART, "cat")
                    .or_else(|| series.find(ns::CHART, "xVal")),
                false,
                cells,
                &location,
                diagnostics,
                budget,
            )?;
            let (vals, val_refs) = values(
                series
                    .find(ns::CHART, "val")
                    .or_else(|| series.find(ns::CHART, "yVal")),
                true,
                cells,
                &location,
                diagnostics,
                budget,
            )?;
            refs.extend(val_refs);
            let mut additional_values = std::collections::BTreeMap::new();
            if let Some(size) = series.find(ns::CHART, "bubbleSize") {
                let (sizes, size_refs) =
                    values(Some(size), true, cells, &location, diagnostics, budget)?;
                additional_values.insert(
                    "Bubble size".into(),
                    sizes.into_iter().map(|(n, _)| n).collect(),
                );
                refs.extend(size_refs);
            }
            result.series.push(ChartSeries {
                additional_values,
                name,
                categories: cat.into_iter().map(|(_, s)| s).collect(),
                values: vals.iter().map(|(n, _)| n.clone()).collect(),
                display_values: vals
                    .into_iter()
                    .map(|(_, s)| if s.is_empty() { None } else { Some(s) })
                    .collect(),
                source_ranges: refs,
            });
        }
    }
    let axes: Vec<_> = plot
        .child_elems()
        .filter(|e| matches!(e.local.as_str(), "catAx" | "valAx" | "dateAx"))
        .collect();
    if axes.len() > 2
        || axes.iter().any(|axis| {
            axis.first_descendant(ns::CHART, "logBase").is_some()
                || axis.find(ns::CHART, "dispUnits").is_some()
        })
    {
        result.incompatible_axes = true;
    }
    result.axis_titles = axes
        .iter()
        .map(|axis| {
            axis.find(ns::CHART, "title")
                .map(|e| {
                    e.descendants(ns::A, "t")
                        .map(Element::text)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        })
        .collect();
    Ok(result)
}

fn charge(count: usize, budget: &mut u64) -> Result<(), ConvertError> {
    *budget = budget.saturating_add(count as u64);
    if *budget > crate::package::limits::MAX_GRID_SLOTS {
        return Err(ConvertError::ResourceLimit {
            limit: "max_grid_slots",
            detail: "workbook chart data exceeds cumulative point limit".into(),
        });
    }
    Ok(())
}
