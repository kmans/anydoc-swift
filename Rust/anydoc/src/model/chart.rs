//! Semantic chart data and stable conversion diagnostics.
use serde::Serialize;

/// A location in the original container, independent of translated UI text.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    /// Package part containing the object.
    pub part: String,
    /// Worksheet name when available.
    pub sheet: Option<String>,
    /// Object title or identifier.
    pub object: Option<String>,
}

/// A structured adaptation or loss notice.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Stable, untranslated identifier.
    pub code: String,
    /// `information` or `warning`.
    pub severity: String,
    /// Origin of the notice.
    pub location: Option<SourceLocation>,
    /// Message interpolation arguments, never executed as markup.
    pub arguments: Vec<String>,
}

/// One source series. Missing values remain missing, never zero-filled.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSeries {
    /// Source series label.
    pub name: String,
    /// Original category labels, in source order.
    pub categories: Vec<String>,
    /// Exact numeric source text, not rounded display labels.
    pub values: Vec<Option<String>>,
    /// Formatted worksheet labels when a range resolves.
    pub display_values: Vec<Option<String>>,
    /// Worksheet formulas identifying the source ranges.
    pub source_ranges: Vec<String>,
    /// Extra numeric dimensions, such as bubble size, preserved in table fallbacks.
    pub additional_values: std::collections::BTreeMap<String, Vec<Option<String>>>,
}

/// A source chart, retained even when only its data can be rendered.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    /// Original OOXML chart kind.
    pub kind: String,
    /// Source title, or empty if absent.
    pub title: String,
    /// Grouped, stacked, percent-stacked, or other source grouping.
    pub grouping: String,
    /// Whether the original bars run horizontally.
    pub horizontal: bool,
    /// Whether the chart combines incompatible plot or axis types.
    pub incompatible_axes: bool,
    /// Source axis titles.
    pub axis_titles: Vec<String>,
    /// All series, without silently removing unsupported ones.
    pub series: Vec<ChartSeries>,
    /// Source location.
    pub location: SourceLocation,
}
