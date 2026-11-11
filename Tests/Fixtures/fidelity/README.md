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
