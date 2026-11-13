# Conversion fidelity corpus

`handbook.docx` and `workbook.xlsx` are synthetic CC0-1.0 examples from the exav
project. `sources.json` records source URLs, license evidence, and SHA-256 hashes.
`baseline-audit.md` records the original Telescopo conversion defects.

`workbook-cells.json` records all 815 nonempty displayed cells and identifies all
147 formulas. Its display values were independently checked against the original
workbook before being frozen as the regression baseline. Raw cached values and
formula text are retained alongside display text. The source's currency-formatted
Units total is intentional and must not be "corrected" by the converter.

`*.expected.md` are reviewed Markdown results using the Telescopo profile.
`generate_fixtures.py` deterministically creates the additional focused regression
inputs. Those authored variants are also dedicated to CC0-1.0. The original two
source files and their hashes must remain unchanged.

`fixture-sha256.json` records every original and focused document input.
`table-layouts.xlsx` isolates explicit headers, ambiguous first rows, merged titles
and covered cells, and cached versus uncached formulas. The workbook golden keeps
the multi-series bar as labeled tables: Mermaid overlays bar series and obscures
earlier values. Its currency-formatted line now also uses a table so the euro
displays remain available alongside the raw values. Its negative-valued pie
remains a table. The source's external hyperlink is retained.

`spreadsheet-followups.xlsx` and its 1904-calendar variant cover inherited column
styles, row overrides, explicit General, safe external links and fragments,
unsupported internal/unsafe/broken links, missing versus valid empty formula
caches, zero and error results, true chartsheets, a missing worksheet part, and
dynamic-name caches. A plain numeric date chart remains Mermaid; the percentage
chart retains raw values and percentage displays in a table. These are original,
deterministic CC0 fixtures; they do not redistribute the third-party demo downloads.

The additional-format fixtures are original CC0 inputs authored for Telescopo on
12 November 2026. `coastal-field-notes.pdf` has three independently drawn Unicode
bullets, an ordinary reference URL, and a three-column table.
`coastal-field-guide.epub` has a matching metadata/visible title and deliberately
reverse-alphabetical chapter filenames to check spine order.
`coastal-records.csv` has 30 cells covering quoted multiline text, commas, literal
Markdown, empty values and Unicode. `legacy-tables-links.ppt` is a small authored
OLE2 deck with a current persist directory, a flagged OfficeArt table, and a
UTF-16 hyperlink range containing an emoji. `generate_additional_formats.py`
recreates the legacy deck without copying any third-party presentation data.

`station-readings.xlsx` is an original CC0 demonstration workbook: three labeled
temperatures and one plain numeric line chart, also used by the website example.
