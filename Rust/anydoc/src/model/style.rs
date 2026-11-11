/// Fully resolved character style. Tri-state deltas exist only during
/// frontend resolution (`shared::delta`); by the time content reaches the
/// model every toggle has a definite value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// Resolved baseline, superscript, or subscript position.
    pub script: Script,
    /// Bold weight.
    pub bold: bool,
    /// Italic or oblique.
    pub italic: bool,
    /// Struck through.
    pub strike: bool,
    /// Monospace, from a code or teletype character style.
    pub code: bool,
}

impl Style {
    /// No toggle set.
    pub const PLAIN: Style = Style {
        script: Script::Baseline,
        bold: false,
        italic: false,
        strike: false,
        code: false,
    };
}

/// Resolved text position, including an explicit baseline reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Script {
    /// Ordinary text.
    #[default]
    Baseline,
    /// Raised text.
    Superscript,
    /// Lowered text.
    Subscript,
}
