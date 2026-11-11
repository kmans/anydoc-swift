//! Semantic regression checks against public-domain source documents.
#![expect(
    clippy::indexing_slicing,
    reason = "Regression assertions index known synthetic fixture shapes."
)]
use anydoc::{
    conversion::{self, MarkdownOptions},
    model::{Block, Chart, ChartSeries, Document, Inline, Script, Style},
};
use pdf_inspector as _;
use serde as _;
use serde_json as _;
fn options() -> MarkdownOptions {
    MarkdownOptions {
        math_scripts: true,
        mermaid_charts: true,
        portable_tables: true,
        asset_references: true,
    }
}
fn chart(kind: &str) -> Chart {
    Chart {
        kind: kind.into(),
        title: "Totals".into(),
        series: vec![ChartSeries {
            name: "Revenue".into(),
            categories: vec!["A".into(), "B".into()],
            values: vec![Some("10".into()), Some("20".into())],
            display_values: vec![Some("10".into()), Some("20".into())],
            ..Default::default()
        }],
        ..Default::default()
    }
}
#[test]
fn handbook_preserves_scripts_code_and_anchored_images() {
    let bytes = include_bytes!("../../../Tests/Fixtures/fidelity/handbook.docx");
    let doc = anydoc::to_document(bytes, None).unwrap();
    let (md, notes) = conversion::render(&doc, options());
    assert!(md.contains("$10^{3}$"));
    assert!(md.contains("$\\mathrm{H}_{2}\\mathrm{O}$"));
    assert!(md.contains("```\nmoor --boat Aurora --length 6.5\nberth 14 free until 06:00\n```"));
    assert!(md.contains("](anydoc-asset:0) The east quay"));
    assert!(md.contains("](anydoc-asset:1)"));
    assert!(!md.contains("dusk.The"));
    assert!(!md.contains("<br>") && !md.contains("<a "));
    assert!(md.contains("](#1--introduction)") || md.contains("](#1-introduction)"));
    assert_eq!(doc.assets.len(), 2);
    assert!(notes.is_empty());
}
#[test]
fn workbook_preserves_titles_and_all_three_charts() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/workbook.xlsx"),
        None,
    )
    .unwrap();
    let (md, notes) = conversion::render(&doc, options());
    assert!(md.starts_with("## Budget\n\n### Harbour budget, first half\n\nAmounts in euros; the totals are formulas.\n\n| Category | Jan |"));
    assert_eq!(
        doc.blocks
            .iter()
            .filter(|b| matches!(b, Block::Chart(_)))
            .count(),
        3
    );
    assert!(md.contains("## Charts\n\n### Spending by category"));
    assert!(md.contains("**Jan**") && md.contains("**Jun**"));
    assert!(md.contains("| Berths let | 11200 |") && md.contains("| Berths let | 24900 |"));
    assert!(md.contains("line \"Net\" [14300, 16100, 16200, 26600, 44000, 52500]"));
    assert!(md.contains("-0.0577489687684149"));
    assert!(md.contains("-5.8%"));
    assert_eq!(
        notes
            .iter()
            .filter(|d| d.code == "chart.tableFallback")
            .count(),
        2
    );
    assert_eq!(
        notes[0].location.as_ref().unwrap().sheet.as_deref(),
        Some("Charts")
    );
}
#[test]
fn unsupported_and_missing_chart_values_become_tables() {
    for kind in ["scatterChart", "bubbleChart", "stockChart"] {
        let c = chart(kind);
        assert!(conversion::chart_mermaid(&c, options()).is_none());
        assert!(conversion::render_chart(&c, options()).contains("| A | 10 |"));
    }
    let mut c = chart("barChart");
    c.series[0].values[0] = None;
    assert!(conversion::chart_mermaid(&c, options()).is_none());
    assert!(conversion::render_chart(&c, options()).contains("Unavailable"));
}
#[test]
fn pie_adapts_only_without_changing_values_or_precision() {
    let mut c = chart("pieChart");
    assert!(
        conversion::chart_mermaid(&c, options())
            .unwrap()
            .starts_with("pie showData")
    );
    c.series[0].values[0] = Some("-1".into());
    assert!(conversion::chart_mermaid(&c, options()).is_none());
    c.series[0].values[0] = Some("1.23456789".into());
    assert!(conversion::chart_mermaid(&c, options()).is_none());
    c.series[0].values[0] = Some("NaN".into());
    assert!(conversion::chart_mermaid(&c, options()).is_none());
}
#[test]
fn incompatible_axes_and_stacking_are_not_reinterpreted() {
    let mut c = chart("barChart");
    c.grouping = "stacked".into();
    assert!(conversion::chart_mermaid(&c, options()).is_none());
    c.grouping.clear();
    c.incompatible_axes = true;
    assert!(conversion::chart_mermaid(&c, options()).is_none());
}
#[test]
fn overlapping_bar_series_fall_back_without_hiding_values() {
    let mut c = chart("barChart");
    assert!(conversion::chart_mermaid(&c, options()).is_some());
    let mut second = c.series[0].clone();
    second.name = "Larger".into();
    second.values = vec![Some("100".into()), Some("200".into())];
    c.series.push(second);
    assert!(conversion::chart_mermaid(&c, options()).is_none());
    let table = conversion::render_chart(&c, options());
    assert!(table.contains("**Larger**") && table.contains("100") && table.contains("10"));
}
#[test]
fn chart_size_limits_keep_all_table_data() {
    let mut c = chart("lineChart");
    c.series[0].categories = (0..201).map(|i| format!("Point {i}")).collect();
    c.series[0].values = vec![Some("1".into()); 201];
    c.series[0].display_values = c.series[0].values.clone();
    assert!(conversion::chart_mermaid(&c, options()).is_none());
    assert!(conversion::render_chart(&c, options()).contains("Point 200"));
}
#[test]
fn chart_labels_cannot_escape_the_mermaid_fence() {
    let mut c = chart("barChart");
    c.title = "\"\n```\n%%{init: malicious}%%".into();
    c.series[0].name = "A\"B".into();
    let text = conversion::chart_mermaid(&c, options()).unwrap();
    assert!(!text.contains("```"));
    assert!(text.contains("#quot;"));
}
#[test]
fn ordinary_combined_profile_preserves_script_meaning_as_text() {
    let doc = Document {
        blocks: vec![Block::Paragraph(vec![
            Inline::plain("10"),
            Inline::Text {
                text: "3".into(),
                style: Style {
                    script: Script::Superscript,
                    ..Style::PLAIN
                },
            },
        ])],
        ..Default::default()
    };
    let (md, _) = conversion::render(&doc, MarkdownOptions::default());
    assert!(md.contains("10^(3)"));
}
#[test]
fn inherited_character_styles_and_explicit_resets_obey_precedence() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/inherited-styles.docx"),
        None,
    )
    .unwrap();
    let (md, _) = conversion::render(&doc, options());
    assert!(md.contains("$\\mathrm{a}^{2}$ b3 c4"), "{md}");
    assert!(md.contains("    let value = 1"));
}
#[test]
fn cyclic_styles_fail_without_partial_output() {
    assert!(
        anydoc::to_document(
            include_bytes!("../../../Tests/Fixtures/fidelity/cyclic-styles.docx"),
            None
        )
        .is_err()
    );
}
#[test]
fn cached_chart_holes_remain_missing_and_fall_back() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/chart-fallbacks.xlsx"),
        None,
    )
    .unwrap();
    let (md, notes) = conversion::render(&doc, options());
    assert!(!md.contains("```mermaid"));
    assert!(md.contains("Unavailable"));
    assert!(notes.iter().any(|n| n.code == "chart.cachedData"));
}
#[test]
fn malformed_chart_does_not_hide_other_sheets_or_charts() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/malformed-chart.xlsx"),
        None,
    )
    .unwrap();
    let (md, notes) = conversion::render(&doc, options());
    assert!(md.contains("## Budget") && md.contains("## Data") && md.contains("## Charts"));
    assert!(
        notes
            .iter()
            .any(|n| n.code == "chart.relationshipUnavailable")
    );
}
#[test]
fn oversized_chart_cache_is_rejected_before_allocation() {
    assert!(matches!(
        anydoc::to_document(
            include_bytes!("../../../Tests/Fixtures/fidelity/oversized-chart-cache.xlsx"),
            None
        ),
        Err(anydoc::ConvertError::ResourceLimit { .. })
    ));
}

#[test]
fn bubble_fallback_retains_x_y_and_size() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/bubble-table.xlsx"),
        None,
    )
    .unwrap();
    let (md, _) = conversion::render(&doc, options());
    assert!(md.contains("Bubble size"));
    assert!(md.contains("| 1 | 11200 | €11,200 | 10 |"), "{md}");
}

#[test]
fn portable_non_heading_anchors_keep_text_and_report_the_adaptation() {
    use anydoc::model::LinkTarget;
    let doc = Document {
        blocks: vec![Block::Paragraph(vec![
            Inline::Anchor("place".into()),
            Inline::plain("Here"),
            Inline::Link {
                content: vec![Inline::plain("Jump")],
                target: LinkTarget::Anchor("place".into()),
            },
        ])],
        ..Document::default()
    };
    let (md, notices) = conversion::render(&doc, options());
    assert!(!md.contains("<a ") && !md.contains("](#place)"));
    assert!(md.contains("Jump"));
    assert!(notices.iter().any(|n| n.code == "anchor.targetUnavailable"));
}

#[test]
fn alternate_image_text_does_not_glue_neighboring_words() {
    use anydoc::model::ImageSource;
    let doc = Document {
        blocks: vec![Block::Paragraph(vec![
            Inline::plain("Before"),
            Inline::Image {
                alt: "picture".into(),
                source: ImageSource::Unavailable,
            },
            Inline::plain("After"),
        ])],
        ..Document::default()
    };
    let (md, _) = conversion::render(&doc, MarkdownOptions::default());
    assert_eq!(md, "Before picture After\n");
}

#[test]
fn extra_visual_details_prevent_a_misleading_simple_chart() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/trendline-table.xlsx"),
        None,
    )
    .unwrap();
    let (md, notes) = conversion::render(&doc, options());
    assert!(!md.contains("bar \"Jan\""));
    assert!(md.contains("€11,200"));
    assert!(notes.iter().any(|n| n.code == "chart.unsupportedDetails"));
}

#[test]
fn table_layouts_preserve_ambiguous_rows_explicit_headers_and_formula_caches() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/table-layouts.xlsx"),
        None,
    )
    .unwrap();
    let (md, _) = conversion::render(&doc, options());
    assert!(
        md.contains("### Quarterly results\n\nFigures supplied by the source"),
        "{md}"
    );
    assert!(md.contains("| Merged value |  | Tail |"), "{md}");
    assert!(
        md.contains(
            "## Ambiguous\n\n| Column 1 | Column 2 |\n| --- | --- |\n| Alpha | 11 |\n| Beta | 22 |"
        ),
        "{md}"
    );
    assert!(
        md.contains(
            "## Explicit\n\n| North | South |\n| --- | --- |\n| Amber | Blue |\n| Green | Violet |"
        ),
        "{md}"
    );
    assert!(
        md.contains("| Cached | Uncached |\n| --- | --- |\n| 4 |  |"),
        "{md}"
    );
    assert!(
        !md.contains("42"),
        "Uncached formulas must not be calculated"
    );
}

#[test]
fn repeated_images_keep_their_original_assets_and_every_anchor() {
    let doc = anydoc::to_document(
        include_bytes!("../../../Tests/Fixtures/fidelity/repeated-images.docx"),
        None,
    )
    .unwrap();
    let (md, _) = conversion::render(&doc, options());
    assert_eq!(doc.assets.len(), 2);
    assert_eq!(md.matches("](anydoc-asset:0)").count(), 2);
    assert_eq!(md.matches("](anydoc-asset:1)").count(), 1);
}
