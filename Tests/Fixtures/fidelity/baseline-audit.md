# Telescopo conversion review — 10 November 2026

Both source files were converted using the native Mac Import as Markdown interface in an isolated Debug copy of Telescopo. The build uses the current shared document importer. Debug access enabled the paid workflow; no purchase was made. The original source files and normal user notes were not modified. The resulting Markdown is copied unchanged into converted/ with its image attachments.

## Result

Not a clean pass for polished conversion marketing. Core content imports and renders, but there are conversion fidelity issues to address or clearly disclose.

## Verified

- XLSX: All 815 nonempty cells in Budget and Data match their source values after applying the displayed currency, percentage, and date formatting. This includes all 147 cached formula results and all 120 sales records. Calculations are saved values, not live spreadsheet formulas in Markdown. Dates normalize from day/month/year display to ISO dates without changing their dates.
- DOCX: Three tables, nested lists, both footnotes, working internal navigation and a working footnote return link were observed. Both extracted PNG files match the original embedded image bytes exactly and render in the app. Both sources retain their published checksums after conversion.

## Findings

1. DOCX superscript/subscript semantics are lost: the source H₂O and 10³ become H2O and 103. The loss is in the saved Markdown, not the reading theme. Superscript loss can change mathematical meaning.
2. DOCX code paragraphs become ordinary paragraphs; quotation styling, highlighting and underline are also flattened. Font, alignment, columns, page furniture and merged-cell layout do not transfer as equivalent Markdown formatting.
3. XLSX Budget gets a synthetic blank Markdown table header; its merged title and description become tall cells in the first column. The real column headings are body cells. The data is preserved, but the result is awkward in the reader.
4. XLSX chart-only Charts sheet and its three charts are omitted. The conversion review showed no warning about the omitted charts/sheet.
5. DOCX embedded images are appended under Imported images rather than retained in their original positions. The import review explicitly warns about this. Image alternative text also joins adjacent prose without a space in one place (dusk.The east quay).

## Source quirks excluded from converter bugs

- The workbook itself formats the Units total as currency; the conversion reproduces that source format.
- The DOCX's bookmark cross-reference has no visible value in the independently rendered original, too; its empty output was not counted as a newly introduced loss.
- The Word fixture deliberately contains tracked changes. The converted text corresponds to accepting insertions and removing deleted text, rather than retaining revision markup.

## Evidence and limits

- handbook-formatting.png and workbook-layout.png show the actual app, with the Lamplight reader theme and light app appearance.
- original-handbook/ contains an independent LibreOffice rendering of all five source pages. original-workbook/ contains the original workbook rendered to PDF.
- data-verification.json records the cell/formula comparison and attachment byte checks.
- This review covers the two specific samples in the Mac app. It is not proof of fidelity across all supported extensions or all documents. No conversion code, release build, website, or public marketing copy was changed.
