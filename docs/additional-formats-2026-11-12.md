# Additional document conversion repairs

Repairs target source semantics, with no document-name or text-content matching.
The native ABI (4), transport schema (2), queue, cancellation, output/input
limits, and PDF OCR policy are unchanged.

- PDF: separate Unicode bullets survive a larger marker font and missing
  marker/text spacing. Nonbold body URL references no longer become headings
  through rarity or sequence heuristics. Existing font/structure headings remain.
- EPUB: a metadata title matching the opening chapter's H1 is emitted once;
  the visible heading, inline styling, and chapter anchors remain. Different
  titles and titles repeated in later chapters remain distinct.
- Legacy PPT: explicitly flagged OfficeArt tables use their cell geometry,
  including one origin for merged cells. Invalid grids retain text with a
  diagnostic. Click links use current-document hyperlink IDs and original
  UTF-16 ranges, including surrogate pairs and paragraph offsets. Unsafe,
  ambiguous, missing, unsupported, or invalid destinations retain text with
  a diagnostic. Legacy chart/image behavior is unchanged.
- CSV: existing multiline-cell flattening now emits one informational notice
  at the first affected cell. All values remain present.

## Verification

`just final-check` passed after reviewing the changed PDF bullet golden and
correcting a new test's assumption about table delimiter spacing. It covered
299 core tests, 24 bridge unit tests, 21 native fidelity tests, 179 PDF Markdown
tests, and 87 public Swift tests. It also ran formatting/lints, licenses,
three-slice native packaging, iOS runtime tests, Simulator tests, Xcode consumers,
public-interface validation, the blocking required-reason API audit, and DocC.

The Release memory probe passed its 512 MiB budget. The exact archive and
native-input fingerprint are recorded in `Native/apple-release.json`.
Cross-toolchain uploaded-candidate qualification is a separate release gate.

The published PDF crate omits 17 external fixtures used by unrelated upstream
font, tagged-PDF and vector-grid tests. An exploratory full upstream run exposed
those missing inputs. No such failure is reported as passing; the added local
PDF gate covers all 179 self-contained Markdown tests, while bridge/public
fixtures verify extraction. Upstream test source is retained unchanged apart
from the new focused tests and the reviewed repairs.

## Real-source recheck

The external FileFormat legacy deck now has all 18 table cells in a six-row,
three-column table and four Fruit Wiki hyperlink ranges. Its seven slide titles,
text, list structure and six unplaced image assets remain. The deck is retained
only in private audit evidence; an authored CC0 OLE2 regression replaces it in
this repository. The PDF has three list items, both data rows, and an ordinary
reference paragraph. The EPUB retains chapter order and one opening title.
The CSV's 30 cells retain their text after the documented line-break adaptation.

## Ownership

Both engines are checksum-pinned published sources with reconstructible patches.
PDF was already a native dependency; vendoring it adds no new runtime engine.
Future repairs remain parser/model/rendering work in this fork, with fixtures.
No app-side reconstruction, hosted service, OCR service, or AI interpretation
was introduced. General format limitations continue to apply.
