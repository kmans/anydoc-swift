# Telescopo conversion profile

The Telescopo fork keeps the pinned anydoc parser, native bridge, Swift API,
fixtures, and native releases together. This release distributes Apple artifacts
for macOS arm64, iOS arm64, and arm64 Simulator. Linux packaging remains upstream
reference material; this fork does not publish a Linux runtime for this version.
The Swift 6.2 consumer minimum is retained. Intel consumers can compile the guarded
Swift adapter, but conversion is unavailable; Apple app architectures must not be
narrowed to accommodate the native framework.

## One conversion

```swift
let result = try await AnyDocConverter().convert(
  from: bytes,
  format: .docx,
  options: .init(
    scripts: .latex,
    charts: .mermaidOrTable,
    tables: .portable,
    images: .assetReferences
  )
)
```

The combined API parses non-PDF documents once and returns Markdown, the original
structured document, embedded assets, and typed diagnostics. PDF preserves its
Markdown-only path and rejects OCR. This API never opts into hosted processing.
The existing `markdown` and `document` methods remain available. General rendering
defaults use readable script notation and chart tables rather than Telescopo's math
and Mermaid profile.

Schema 2 and ABI 4 add script position, semantic charts, source locations,
diagnostics, and optional combined Markdown. Swift validates both output limits,
manifest metadata, assets, schema, and result ownership before exposing a result.
The native owner is released on every exit. All operations share the original FIFO
queue and cancellation rules. Native work already underway cannot be interrupted.

## Fidelity rules

DOCX resolves inherited and direct script positions, including baseline resets,
and recognized code/monospace styles. LaTeX retains an expression's adjacent base
and chemical suffix. Embedded image references stay near their anchors; unplaced
and unsupported assets receive diagnostics. Relative placement does not reproduce
Word's floating layout, pages, columns, headers, or footers.

XLSX/XLSM retains worksheet order and chart-only sheets. Conservative title
normalization lifts full-width merged titles over multi-column data. Explicit
Excel table headers take priority; ambiguous content stays in the table. Covered
merged cells remain empty. Values, cached formulas, and formatted display text
are preserved; formulas are not evaluated. Effective number formats resolve cell,
custom row and column defaults in that order, including explicit General resets.
Formatted numeric data rows are not inferred to be headers. A formula without a
saved result has a plain-text unavailable marker and a source-cell diagnostic;
valid empty-string results, zero and Excel error values remain distinct.

Dedicated Excel chartsheets follow the same drawing/chart relationships as
worksheet-hosted charts. An unreadable visible sheet produces a partial-import
diagnostic; fatal resource-limit errors still fail the conversion. Hidden-sheet,
row and column visibility behavior is unchanged.

Safe external spreadsheet hyperlinks retain their Markdown destinations and
fragments. Internal cell targets that portable Markdown cannot address remain
text with a diagnostic. Unsafe, missing or unsupported destinations are reported;
no relationship executes or fetches a remote resource.

Charts retain original series, categories, axis titles, numeric strings, source
ranges, and extra dimensions such as bubble size. Worksheet values take priority;
chart caches are used only when the referenced worksheet range cannot be resolved and are identified in diagnostics. Blank points in a readable range remain unavailable, even if an old chart cache contains values. No
external relationship is fetched. Bar and line use Mermaid XY charts; area can
use a line; suitable pie/doughnut data uses a pie. Stacking, incompatible/logarithmic
axes, unsupported types, missing points, precision loss, and oversized diagrams
fall back to complete data tables. Multiple bar series also use tables: the bundled Mermaid renderer overlays the bars and can hide earlier series. Diagram generation is limited to 200 categories
and eight series. Source chart data has a cumulative point budget in addition to
the parser's existing archive, XML, grid, and expansion limits. Missing values
never become zero and negative pie values never become absolute values.

Numeric chart caches honor their number-format codes and the chart/workbook date
system. Date categories use unambiguous ISO dates, including valid day zero in
the 1904 calendar. Axis formats are retained when they can be assigned faithfully.
Percentage, currency and other displays a Mermaid numeric axis cannot express
use tables containing both exact raw values and formatted displays. Unavailable
chart formats receive a diagnostic. Multiple-axis formats are never indiscriminately
applied to all series.

Portable output contains no HTML tags. Non-heading internal anchors cannot always
be represented in portable Markdown; affected links retain their text and produce
a diagnostic. Heading links and footnotes retain their Markdown targets.

## Adding a future fix

1. Add a minimal fixture and an observable failing semantic assertion.
2. Repair the source-format parser or extend the information-preserving model.
3. Add a deterministic rendering rule, with a table/text fallback and diagnostic
   when the destination cannot express the source meaning.
4. Extend transport and Swift validation if the model changes; bump the ABI/schema
   for incompatible changes. Keep unsafe operations in the existing adapter.
5. Review golden Markdown, preserve source bytes and all values, and run platform
   and memory checks before releasing a new immutable native archive.

Do not add application-side regular expressions that guess lost source structure.
Do not add model protocols, public registries, or format-specific public converters.

The native parser is the checksum-pinned anydoc 0.2.4 crate, revision
`42bf1c5ecdde9eb0d96d6bd75a9e6698cf93b14c`. `Rust/anydoc/VENDORED-SOURCE.json`
records its provenance. `Native/patches/anydoc-0.2.4-fidelity.patch` reconstructs the
maintained source from that archive. `just check-vendored-source` verifies the file
and patch hashes offline. After an intentional source change, review the diff and
refresh using the original verified archive:

```sh
python3 Scripts/check-vendored-source.py \
  --archive .build/sources/anydoc-0.2.4.crate --refresh
```

## Regression corpus

`Tests/Fixtures/fidelity` holds the exav CC0 handbook and workbook, hashes and
source records, the original fidelity audit, golden Markdown, and an independent
baseline of 815 displayed cells and 147 cached formula results. The deterministic
fixture generator adds inherited styles, baseline resets, cyclic styles, chart
cache holes, malformed references, oversized caches, and bubble dimensions.
Source files are immutable inputs. Rebuild generated fixtures intentionally and
review changed expectations rather than accepting snapshots automatically.


## Image destination contract

Combined conversions expose opaque per-conversion destinations on each asset's
`markdownReference`. The renderer assigns these only to image AST nodes, so a
literal Markdown example in source code cannot become an imported attachment.
Consumers use the returned destination rather than constructing strings from an
asset ID. Telescopo also uses unique intermediate destinations until its final
collision-safe save. The literal-reference fixture exercises both boundaries.

Release candidates record their complete native input fingerprint and archive
checksum with `python3 Scripts/record-apple-release.py`. Update the package
checksum to that recorded value before the compatibility workflow, and publish
only the exact checked candidate. The vendored parser's pinned development lock
is separate from the bridge's release graph; `just test-core` runs its unit tests.

## Additional document formats

PDF Unicode bullet markers are recognized even when text extraction supplies no
space after the marker. A larger marker glyph cannot merge neighboring list
items into a heading. Ordinary nonbold URL reference lines are excluded from
the rarity/sequence heading heuristics; explicit structure-tree and font-size
headings retain priority. PDF text extraction, OCR rejection, and image policy
remain unchanged. The exact published PDF engine is vendored and patched beside
anydoc so these repairs ship with the bridge.

EPUB keeps the first readable chapter's visible H1, including inline formatting,
when it matches the metadata title after case/whitespace normalization. Different
titles remain distinct; a later chapter cannot suppress the book's opening title.
CSV retains the existing portable-table flattening of cell line breaks and now
reports one informational `table.cellLineBreakFlattened` notice at the first
affected source cell.

Legacy PPT resolves hyperlink IDs from the current DocumentContainer, and click
text ranges from the original UTF-16 text. Styles and paragraph offsets survive;
unsafe/missing targets, invalid/overlapping ranges and unrepresentable actions
keep text and produce `presentation.linkUnavailable`. Only explicitly flagged
OfficeArt tables with complete, nonoverlapping geometry become structured tables.
Merged cells retain one origin; ambiguous groups keep their text and produce
`presentation.tableFlattened`. Arbitrary aligned text boxes are never guessed
to be tables. Legacy charts and image placement are unchanged. No relationship
executes an action or fetches a remote resource.
