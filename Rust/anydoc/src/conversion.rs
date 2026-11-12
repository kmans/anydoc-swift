//! Explicit rendering policy over the parsed document model.
use crate::model::{Block, Chart, Diagnostic, Document, ImageSource, Inline, Script};
use std::collections::{HashMap, HashSet};

/// Optional portable representations; the ordinary Markdown API keeps its defaults.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownOptions {
    /// Preserve superscripts and subscripts as LaTeX math.
    pub math_scripts: bool,
    /// Adapt faithful charts to Mermaid, otherwise retain their data as tables.
    pub mermaid_charts: bool,
    /// Avoid HTML in table cells and anchor output.
    pub portable_tables: bool,
    /// Emit embedded images at their anchors using stable asset placeholders.
    pub asset_references: bool,
}

/// Serialize once-parsed content and return presentation diagnostics.
pub fn render(document: &Document, options: MarkdownOptions) -> (String, Vec<Diagnostic>) {
    render_with_references(document, options, &HashMap::new())
}

/// Render explicit, caller-owned asset destinations without replacing literal source text.
pub fn render_with_references(
    document: &Document,
    options: MarkdownOptions,
    references: &HashMap<usize, String>,
) -> (String, Vec<Diagnostic>) {
    let mut doc = Document {
        blocks: document.blocks.clone(),
        notes: document.notes.clone(),
        assets: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut diagnostics = document.diagnostics.clone();
    if options.portable_tables {
        let anchors = crate::render::markdown::anchors::resolve_anchors(document);
        collect_anchor_notices(&document.blocks, &anchors, &mut diagnostics);
        for note in &document.notes {
            collect_anchor_notices(&note.blocks, &anchors, &mut diagnostics);
        }
    }
    let mut used_assets = HashSet::new();
    for asset in &document.assets {
        if image_extension(&asset.media_type).is_none() {
            diagnostics.push(Diagnostic {
                code: "image.unsupportedFormat".into(),
                severity: "warning".into(),
                location: Some(crate::model::SourceLocation {
                    part: asset.origin_part.clone(),
                    ..Default::default()
                }),
                arguments: vec![],
            });
            disable_asset(&mut doc.blocks, asset.id);
            for note in &mut doc.notes {
                disable_asset(&mut note.blocks, asset.id);
            }
        }
    }
    adapt_blocks(&mut doc.blocks, options, &mut diagnostics, &mut used_assets);
    for note in &mut doc.notes {
        adapt_blocks(
            &mut note.blocks,
            options,
            &mut diagnostics,
            &mut used_assets,
        );
    }
    if options.asset_references {
        let unplaced: Vec<_> = document
            .assets
            .iter()
            .filter(|a| !used_assets.contains(&a.id.0))
            .collect();
        if !unplaced.is_empty() {
            doc.blocks
                .push(Block::heading(2, vec![Inline::plain("Unplaced images")]));
            for asset in unplaced {
                if image_extension(&asset.media_type).is_some() {
                    doc.blocks.push(Block::Paragraph(vec![Inline::Image {
                        alt: String::new(),
                        source: ImageSource::Asset(asset.id),
                    }]));
                    diagnostics.push(Diagnostic {
                        code: "image.unplaced".into(),
                        severity: "warning".into(),
                        location: Some(crate::model::SourceLocation {
                            part: asset.origin_part.clone(),
                            ..Default::default()
                        }),
                        arguments: vec![],
                    });
                }
            }
        }
    }
    (
        if !options.math_scripts
            && !options.mermaid_charts
            && !options.portable_tables
            && !options.asset_references
        {
            crate::render::markdown::document_to_markdown(&doc)
        } else {
            crate::render::markdown::document_to_markdown_with_references(&doc, options, references)
        },
        diagnostics,
    )
}

/// Extension for a supported embedded image media type.
pub fn image_extension(media_type: &str) -> Option<&'static str> {
    match media_type.to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/tiff" => Some("tiff"),
        "image/bmp" => Some("bmp"),
        _ => None,
    }
}

fn adapt_blocks(
    blocks: &mut [Block],
    options: MarkdownOptions,
    diagnostics: &mut Vec<Diagnostic>,
    assets: &mut HashSet<usize>,
) {
    for block in blocks {
        match block {
            Block::Chart(chart) => {
                if chart_mermaid(chart, options).is_none() {
                    diagnostics.push(Diagnostic {
                        code: if chart.series.iter().all(|s| s.values.is_empty()) {
                            "chart.dataUnavailable"
                        } else {
                            "chart.tableFallback"
                        }
                        .into(),
                        severity: "warning".into(),
                        location: Some(chart.location.clone()),
                        arguments: vec![],
                    });
                } else if matches!(chart.kind.as_str(), "areaChart" | "doughnutChart") {
                    diagnostics.push(Diagnostic {
                        code: "chart.presentationAdapted".into(),
                        severity: "information".into(),
                        location: Some(chart.location.clone()),
                        arguments: vec![],
                    });
                }
            }
            Block::Paragraph(inlines)
            | Block::Heading {
                content: inlines, ..
            } => adapt_inlines(inlines, options, assets),
            Block::BlockQuote(inner) => adapt_blocks(inner, options, diagnostics, assets),
            Block::List(list) => {
                for item in &mut list.items {
                    adapt_blocks(&mut item.blocks, options, diagnostics, assets);
                }
            }
            Block::Table(table) => {
                for row in &mut table.grid {
                    for slot in row {
                        if let crate::model::CellSlot::Origin(cell) = slot {
                            adapt_blocks(&mut cell.blocks, options, diagnostics, assets);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn adapt_inlines(inlines: &mut Vec<Inline>, options: MarkdownOptions, assets: &mut HashSet<usize>) {
    for inline in inlines.iter_mut() {
        match inline {
            Inline::Link { content, .. } => adapt_inlines(content, options, assets),
            Inline::Image {
                source: ImageSource::Asset(id),
                ..
            } => {
                assets.insert(id.0);
            }
            _ => {}
        }
    }
    if !options.math_scripts {
        for inline in inlines.iter_mut() {
            if let Inline::Text { text, style } = inline {
                if style.script != Script::Baseline {
                    *text = format!(
                        "{}({text})",
                        if style.script == Script::Superscript {
                            '^'
                        } else {
                            '_'
                        }
                    );
                    style.script = Script::Baseline;
                }
            }
        }
        return;
    }
    // Group a plain alphanumeric base, its script, and adjacent chemical suffix.
    // Whitespace/punctuation stays outside math; no semantic values are guessed.
    let source = std::mem::take(inlines);
    for inline in source {
        match inline {
            Inline::Text { text, style } if style.script != Script::Baseline => {
                let marker = if style.script == Script::Superscript {
                    '^'
                } else {
                    '_'
                };
                let mut base = String::new();
                let mut base_is_math = false;
                if let Some(Inline::Text {
                    text: previous,
                    style: prior_style,
                }) = inlines.last_mut()
                {
                    if prior_style.script == Script::Baseline && !prior_style.code {
                        let split = previous
                            .char_indices()
                            .rev()
                            .find(|(_, c)| !c.is_alphanumeric())
                            .map_or(0, |(i, c)| i + c.len_utf8());
                        base = previous.split_off(split);
                    }
                } else if let Some(Inline::Math(previous)) = inlines.last_mut() {
                    base = std::mem::take(previous);
                    base_is_math = true;
                    inlines.pop();
                }
                let expression = format!(
                    "{}{}{{{}}}",
                    if base_is_math { base } else { math_text(&base) },
                    marker,
                    math_text(&text)
                );
                inlines.push(Inline::Math(expression));
            }
            Inline::Text { mut text, style }
                if matches!(inlines.last(), Some(Inline::Math(_)))
                    && style.script == Script::Baseline
                    && !style.code =>
            {
                let split = text
                    .char_indices()
                    .find(|(_, c)| !c.is_alphanumeric())
                    .map_or(text.len(), |(i, _)| i);
                let suffix = text[..split].to_string();
                if let Some(Inline::Math(previous)) = inlines.last_mut() {
                    previous.push_str(&math_text(&suffix));
                }
                text.drain(..split);
                if !text.is_empty() {
                    inlines.push(Inline::Text { text, style });
                }
            }
            other => inlines.push(other),
        }
    }
}

fn math_text(text: &str) -> String {
    let escaped: String = text
        .chars()
        .map(|c| match c {
            '\\' => "\\backslash{}".into(),
            '{' | '}' | '$' | '%' | '#' | '&' | '_' => format!("\\{c}"),
            '^' => "\\textasciicircum{}".into(),
            '~' => "\\textasciitilde{}".into(),
            _ => c.to_string(),
        })
        .collect();
    if text
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        escaped
    } else {
        format!("\\mathrm{{{escaped}}}")
    }
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("#quot;"),
            '&' => out.push_str("#38;"),
            '<' => out.push_str("#60;"),
            '>' => out.push_str("#62;"),
            '`' => out.push_str("#96;"),
            '#' => out.push_str("#35;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Select only a representation that keeps the chart's quantitative meaning.
pub fn chart_mermaid(chart: &Chart, options: MarkdownOptions) -> Option<String> {
    if !options.mermaid_charts
        || chart.incompatible_axes
        || chart.series.is_empty()
        || chart.series.len() > 8
        // Mermaid overlays bar series instead of grouping them; later bars can
        // completely hide earlier values. Retain all series as labeled tables.
        || (chart.kind == "barChart" && chart.series.len() > 1)
        || matches!(chart.grouping.as_str(), "stacked" | "percentStacked")
    {
        return None;
    }
    if chart.series.len() > 1 && chart.series.iter().any(|s| s.name.is_empty()) {
        return None;
    }
    let categories = &chart.series[0].categories;
    if categories.is_empty()
        || categories.len() > 200
        || chart.series.iter().any(|s| {
            !s.additional_values.is_empty()
                || s.categories != *categories
                || s.values.len() != categories.len()
        })
    {
        return None;
    }
    let values: Option<Vec<Vec<f64>>> = chart
        .series
        .iter()
        .map(|s| {
            s.values
                .iter()
                .map(|v| v.as_ref()?.parse::<f64>().ok().filter(|n| n.is_finite()))
                .collect()
        })
        .collect();
    let values = values?;
    // Mermaid's numeric axes/pie labels cannot carry arbitrary spreadsheet
    // formats. Keep exact numbers plus formatted displays in the table instead
    // of silently turning percentages, currency or units into bare quantities.
    if chart.series.iter().any(|s| {
        s.display_values
            .iter()
            .zip(&s.values)
            .any(
                |(display, raw)| match (display.as_deref(), raw.as_deref()) {
                    (Some(a), Some(b)) if a != b => match (a.parse::<f64>(), b.parse::<f64>()) {
                        (Ok(a), Ok(b)) => a != b,
                        _ => true,
                    },
                    _ => false,
                },
            )
    }) {
        return None;
    }
    if matches!(chart.kind.as_str(), "pieChart" | "doughnutChart") {
        if chart.series.len() != 1
            || values[0].iter().any(|n| *n < 0.0)
            || !values[0].iter().any(|n| *n > 0.0)
        {
            return None;
        }
        // The bundled pie renderer displays two decimals. Do not silently round.
        if values[0]
            .iter()
            .any(|n| (n * 100.0 - (n * 100.0).round()).abs() > 1e-8)
        {
            return None;
        }
        let mut out = format!("pie showData\n    title {}\n", quoted(&chart.title));
        for (label, value) in categories.iter().zip(&chart.series[0].values) {
            out.push_str(&format!("    {} : {}\n", quoted(label), value.as_ref()?));
        }
        return Some(out);
    }
    let plot = match chart.kind.as_str() {
        "barChart" => "bar",
        "lineChart" | "areaChart" => "line",
        _ => return None,
    };
    let mut out = format!(
        "xychart-beta{}\n",
        if chart.horizontal { " horizontal" } else { "" }
    );
    if !chart.title.is_empty() {
        out.push_str(&format!("    title {}\n", quoted(&chart.title)));
    }
    let x_title = chart
        .axis_titles
        .first()
        .filter(|s| !s.is_empty())
        .map(|s| format!("{} ", quoted(s)))
        .unwrap_or_default();
    out.push_str(&format!(
        "    x-axis {x_title}[{}]\n",
        categories
            .iter()
            .map(|v| quoted(v))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if let Some(title) = chart.axis_titles.last().filter(|s| !s.is_empty()) {
        out.push_str(&format!("    y-axis {}\n", quoted(title)));
    }
    for series in &chart.series {
        let numbers = series
            .values
            .iter()
            .map(|v| v.as_deref().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(", ");
        if series.name.is_empty() {
            out.push_str(&format!("    {plot} [{numbers}]\n"));
        } else {
            out.push_str(&format!(
                "    {plot} {} [{numbers}]\n",
                quoted(&series.name)
            ));
        }
    }
    Some(out)
}

/// Render a chart or all of its source data, without dropping unsupported series.
pub fn render_chart(chart: &Chart, options: MarkdownOptions) -> String {
    if let Some(source) = chart_mermaid(chart, options) {
        return format!("```mermaid\n{source}```");
    }
    let mut blocks = Vec::new();
    if !chart.title.is_empty() {
        blocks.push(format!("### {}", plain(&chart.title)));
    }
    for (index, series) in chart.series.iter().enumerate() {
        if series.name.is_empty() && chart.series.len() > 1 {
            blocks.push(format!("**Series {}**", index + 1));
        }
        if !series.name.is_empty() {
            blocks.push(format!("**{}**", plain(&series.name)));
        }
        if series.values.is_empty()
            && series.categories.is_empty()
            && series.additional_values.is_empty()
        {
            continue;
        }
        let numbered = series.categories.is_empty();
        let formatted = series
            .display_values
            .iter()
            .zip(&series.values)
            .any(|(a, b)| a != b);
        let category_title = if numbered {
            "Point"
        } else {
            chart
                .axis_titles
                .first()
                .filter(|s| !s.is_empty())
                .map(String::as_str)
                .unwrap_or("Category")
        };
        let value_title = chart
            .axis_titles
            .last()
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .unwrap_or("Value");
        let mut table = format!(
            "| {} | {} |{}\n| --- | --- |{}",
            plain(category_title),
            plain(value_title),
            if formatted { " Display value |" } else { "" },
            if formatted { " --- |" } else { "" }
        );
        for name in series.additional_values.keys() {
            table.insert_str(
                table.find('\n').unwrap_or(table.len()),
                &format!(" {} |", plain(name)),
            );
        }
        for _ in series.additional_values.keys() {
            table.push_str(" --- |");
        }
        let rows = series
            .additional_values
            .values()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .max(series.values.len())
            .max(series.categories.len());
        for i in 0..rows {
            let category = series.categories.get(i).cloned().unwrap_or_else(|| {
                if numbered {
                    (i + 1).to_string()
                } else {
                    String::new()
                }
            });
            let value = series
                .values
                .get(i)
                .and_then(Option::as_deref)
                .unwrap_or("Unavailable");
            table.push_str(&format!("\n| {} | {} |", plain(&category), plain(value)));
            if formatted {
                table.push_str(&format!(
                    " {} |",
                    plain(
                        series
                            .display_values
                            .get(i)
                            .and_then(Option::as_deref)
                            .unwrap_or("Unavailable")
                    )
                ));
            }
            for values in series.additional_values.values() {
                table.push_str(&format!(
                    " {} |",
                    plain(
                        values
                            .get(i)
                            .and_then(Option::as_deref)
                            .unwrap_or("Unavailable")
                    )
                ));
            }
        }
        blocks.push(table);
    }
    if chart.series.iter().all(|s| s.values.is_empty()) {
        blocks.push("Chart data is unavailable in the source document.".into());
    }
    blocks.join("\n\n")
}
fn plain(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_control() {
                " ".chars().collect::<Vec<_>>()
            } else if "\\|`*_{}[]<>#!".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

fn disable_asset(blocks: &mut [Block], id: crate::model::AssetId) {
    fn inlines(values: &mut [Inline], id: crate::model::AssetId) {
        for value in values {
            match value {
                Inline::Image { source, .. } if *source == ImageSource::Asset(id) => {
                    *source = ImageSource::Unavailable
                }
                Inline::Link { content, .. } => inlines(content, id),
                _ => {}
            }
        }
    }
    for block in blocks {
        match block {
            Block::Paragraph(v) | Block::Heading { content: v, .. } => inlines(v, id),
            Block::BlockQuote(v) => disable_asset(v, id),
            Block::List(v) => {
                for item in &mut v.items {
                    disable_asset(&mut item.blocks, id);
                }
            }
            Block::Table(v) => {
                for row in &mut v.grid {
                    for slot in row {
                        if let crate::model::CellSlot::Origin(c) = slot {
                            disable_asset(&mut c.blocks, id);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_anchor_notices(
    blocks: &[Block],
    anchors: &crate::render::markdown::anchors::AnchorMap,
    notices: &mut Vec<Diagnostic>,
) {
    fn inlines(
        values: &[Inline],
        anchors: &crate::render::markdown::anchors::AnchorMap,
        notices: &mut Vec<Diagnostic>,
    ) {
        for value in values {
            if let Inline::Link { content, target } = value {
                if let crate::model::LinkTarget::Anchor(id) = target {
                    if anchors.portable_fragment(id).is_none()
                        && !notices.iter().any(|n| {
                            n.code == "anchor.targetUnavailable" && n.arguments.contains(id)
                        })
                    {
                        notices.push(Diagnostic {
                            code: "anchor.targetUnavailable".into(),
                            severity: "information".into(),
                            location: None,
                            arguments: vec![id.clone()],
                        });
                    }
                }
                inlines(content, anchors, notices);
            }
        }
    }
    for block in blocks {
        match block {
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                inlines(content, anchors, notices)
            }
            Block::BlockQuote(inner) => collect_anchor_notices(inner, anchors, notices),
            Block::List(list) => {
                for item in &list.items {
                    collect_anchor_notices(&item.blocks, anchors, notices);
                }
            }
            Block::Table(table) => {
                for row in &table.grid {
                    for slot in row {
                        if let crate::model::CellSlot::Origin(cell) = slot {
                            collect_anchor_notices(&cell.blocks, anchors, notices);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
