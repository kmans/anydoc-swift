//! Reproducible developer review of a source document with the Telescopo profile.
use pdf_inspector as _;
use serde as _;
use std::{env, fs};
#[expect(
    clippy::print_stdout,
    reason = "The developer CLI reports one summary per source document."
)]
fn main() {
    for source in env::args().skip(1) {
        let bytes = fs::read(&source).expect("read source");
        let document = anydoc::to_document(&bytes, None).expect("parse source");
        let options = anydoc::conversion::MarkdownOptions {
            math_scripts: true,
            mermaid_charts: true,
            portable_tables: true,
            asset_references: true,
        };
        let (markdown, diagnostics) = anydoc::conversion::render(&document, options);
        fs::write(format!("{source}.md"), markdown).expect("write review Markdown");
        fs::write(
            format!("{source}.diagnostics.json"),
            serde_json::to_vec_pretty(&diagnostics).expect("encode report"),
        )
        .expect("write diagnostics");
        println!(
            "{source}: {} blocks, {} assets, {} diagnostics",
            document.blocks.len(),
            document.assets.len(),
            diagnostics.len()
        );
    }
}
