# Reviewed native dependency sources

Both native engines are committed under `Rust/` with their published crate
identity, archive checksum, license, and a reviewed patch. Native builds and
parser tests use those same sources through Cargo's local patch mechanism.
No Cargo registry cache is edited.

- `anydoc` 0.2.4: document parsing, the structured model, and rendering.
- `pdf-inspector` 1.24.0: PDF extraction and Markdown layout interpretation.
  Archive SHA-256:
  `e22dc125a533d212c847c8c85e4fcb7358f4384869ef76b2b8721f039b1b633a`.

`Scripts/check-vendored-source.py` verifies anydoc by default and PDF with
`--crate pdf-inspector`. Each `*-fidelity.patch` reconstructs the committed
source from the exact published archive; its `*-fidelity-files.json` records
archive, patch, and file checksums. Refresh only after reviewing a change, with
`--archive <original.crate> --refresh`. Both engines are checked by `just ci-rust`.

The PDF patch retains the three iOS-only guards from the historical
`pdf-inspector-1.24.0-ios-cmaps.patch`: the optional `PDF_INSPECTOR_BCMAPS_DIR`
filesystem override is excluded on iOS, including Simulator, while embedded
CMaps remain available. Other targets keep that upstream override. The former
staging script is no longer needed; the producer validates the committed PDF
manifest path and source checksums before compiling. The separate binary CMap
decoder limitation is unchanged.

The published PDF crate omits external fixtures used by 17 upstream font,
structure-tree, and vector-grid tests. Its 179 Markdown tests run as the local
PDF gate; the full upstream unit run is recorded separately, without treating
missing fixture failures as passing. Core and bridge tests also exercise PDF
extraction and real document conversion. Upstream licenses remain unchanged.
