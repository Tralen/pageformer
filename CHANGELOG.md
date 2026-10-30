# Change Log

## Version History

*Major updates and feature releases.*

### 0.19.0

- Added `rule` to each heading level `h1` to `h6`: a line drawn below the heading text, styled with `thickness`, `color`, `spacing_before` (the gap above the line) and `spacing_after`. A rule is not inherited from the base heading map.
- The technical example draws a solid bar behind its level-one headings and a rule below its level-two headings. The regenerated PDF, ODT and DOCX keep three pages; the TXT golden and the shared examples are unchanged.
- The DOCX gap of a heading rule is clamped to the OOXML range of 0 to 31 points.

### 0.18.0

- Added the `formatting.rule` section to the template: `thickness`, `color`, `spacing_before` and `spacing_after` style the thematic break in every format. The defaults are `0.5pt`, `#000000` and each format's own spacing.
- Added `background` to `formatting.heading` and to `h1` to `h6`: a solid fill across the full text width with no horizontal margins, with the spacing kept outside the fill.
- The ODT and DOCX rules now follow the template. Their default thickness changes from `0.75pt` to `0.5pt` to match the PDF, and DOCX border widths are clamped to the OOXML range.
- Regenerated the ODT examples; their only change is the rule style, because the examples contain no thematic break.
- Added tests for the new keys, the PDF source, the ODT and DOCX parts and a PDF render with rules and backgrounds.

### 0.17.1

- Removed automatic hyphenation from the DOCX settings: LibreOffice asked for a dictionary even when the system package was installed. The document language stays in the styles, so the editor can enable hyphenation itself.
- Moved the DOCX header band up so the page number clears the letterhead rule, and switched line spacing to "at least" so a tight heading line no longer clips the top of the title.
- Table cell paragraphs now reset the paragraph spacing and the line, so rows no longer come out at double height.
- Regenerated the DOCX examples.

### 0.17.0

- Added `--format docx` and the DOCX backend: an Office Open XML package with the template's page layout, fonts, headings, lists, tables, letterhead, running header and footer, footnotes, hyperlinks, images and properties.
- Every list gets its own numbering instance with a start override; table header rows repeat; the template supplies borders, header fill and cell padding; the letterhead floats behind the header.
- The package is a stored ZIP assembled by the internal `src/zip.rs`; all relationship ids, numbering ids, drawing ids and property dates are fixed, so two runs are byte-identical.
- Shared the XML escaping, the Typst spacing model and the image size reader between the office backends in `src/xml.rs`, `src/metrics.rs` and `src/image.rs`.
- Added DOCX examples and structural tests, plus a LibreOffice smoke test documented in `docs/verification.md`.

### 0.16.2

- Enabled hyphenation with the frontmatter language on paragraphs, headings and cells, because Typst hyphenates and the ODT line breaks otherwise drift; the Embargos item 12 now stays on page 2, as in the PDF.
- The running header and footer use the paragraph face, so the page number is 12pt instead of a fixed 9pt, and reserve a 1.5cm band; the number now sits above the letterhead rule.
- Regenerated the ODT examples; the page counts remain 4, 4 and 3 and the page starts match the PDF.

### 0.16.1

- Matched the ODT spacing to the PDF: the ODF line height is the leading plus the cap height Typst uses, paragraph spacing subtracts the leading, heading margins account for the Writer line box, and tight list items carry no paragraph margin.
- Table cells no longer inherit paragraph spacing, and the leading is absorbed into the vertical padding, so rows have the same height as the PDF.
- A running header or footer reserves its band from the page margin, so the text area keeps the template margins and the header sits over the letterhead instead of pushing the body.
- The three ODT examples now match the PDF page counts (4, 4 and 3) and were regenerated.

### 0.16.0

- Added `--format odt` and the ODT backend: an OpenDocument text package with the template's page layout, fonts, headings, lists, tables, letterhead background, running header and footer, footnotes, links, images and metadata.
- The package is a stored ZIP with `mimetype` first and fixed timestamps, written by an internal `src/zip.rs`, so two runs are byte-identical and no compression dependency is added.
- Extracted the asset loader to `src/assets.rs`, shared by the PDF and ODT backends.
- Alerts become block quotes with a visible `[!KIND]` label, footnote definitions become inline notes, and confidential values never reach the metadata.
- Added ODT examples and structural tests with the `zip` and `roxmltree` dev-dependencies, plus a LibreOffice smoke test documented in `docs/verification.md`.

### 0.15.0

- Added `--format txt` and the plain-text backend: UTF-8 with LF line endings, no automatic wrapping, one blank line between blocks, Markdown-style list markers, tab-separated tables with a separator row, quotes prefixed with `>` and a space, `label (destination)` links, `[alt]` images, footnotes collected at the end and a form feed for an explicit page break.
- Introduced the renderer boundary: `Renderer` returns backend-neutral bytes plus warnings, `PdfRenderer` moved the PDF backend under it, and the TXT backend is total.
- Added text goldens for the office and technical examples, compared byte for byte, and regenerated them.
- Documented that the INI values file stays outside the Markdown and YAML sources, so editor `private_files` and `private_paths` rules can keep it out of agent file reads.

### 0.14.0

- Replaced the `NAME=value` confidential values file with an INI-style `NAME = value` format, which is easier to read and receives syntax highlighting in editors; the example is now `assets/examples/confidencial/embargos.ini`.
- Whitespace around the name, the `=` and the value is trimmed; `;` also starts a comment, alongside `#`; a value keeps every `=` after the first and any `#` or `;` that does not start the line; sections, quotes and escapes are rejected.
- The compact `NAME=value` form still parses, so existing files keep working.

### 0.13.2

- Shrank the development build: the dev profile keeps line tables only, dependencies carry no debug info and incremental compilation is off, so the main test binary dropped from about 540 MB to 155 MB and the debug tree from 88 GB to about 1.7 GB.
- Documented the build cache and its cleanup in the README.

### 0.13.1

- Moved the office examples to `assets/examples/escritorio/`, their fictional confidential values to `assets/examples/confidencial/embargos.env`, and the generated PDFs to `assets/examples/publicacao/`.
- Applied confidential fields to `Embargos.md` for the fictional company names, CNPJ, address, partner name, RG, CPF, court, process number, lawyers and case identifiers.
- Regenerated the office PDFs; both still pass veraPDF as PDF/A-2b.
- Documented that the committed values file is fictional and needs `chmod 600` after a checkout.

### 0.13.0

- Renamed the secrets phase to confidentiality and implemented it. A `{NAME}` in the body is a confidential field.
- Added `--confidential-file`, a `NAME=value` file outside the source tree with `0600` permissions on Unix; the file overrides the `PAGEFORMER_CONFIDENTIAL_<NAME>` environment variables.
- Added the document model resolution: text placeholders are substituted once, code spans and blocks, destinations, titles and frontmatter are excluded, and an unknown field is fatal.
- A document with confidential fields requires `--output` outside its source tree; `--check` lists names only.
- Known values are redacted from warnings and errors before stderr.
- A confidential field in frontmatter or in running header and footer content is now an error.
- Superscript and subscript are deferred until after 1.0.0; the roadmap records the decision.

### 0.12.1

- Added tests for every inline construct: reference links, angle autolinks, entities, escapes, single and double tilde strikethrough, and a rendering test with every inline variant.
- Documented the inline contract and its deliberate exclusions: math, superscript, subscript, highlight, wikilinks, inline HTML and smart punctuation.

### 0.12.0

- Added `page_break_before` to `formatting.heading` and to the `h1` to `h6` overrides. The break is weak, so a heading already at the top of a page does not create a blank page.
- Closed phase 2 of the roadmap. Typst already avoids widows and orphans, so no template field is exposed; PDF/A-2b remains the declared profile, since PDF/A-2u would only change the conformance identifier without a user need.
- The technical example starts each first-level heading on a new page; regenerated its PDF.

### 0.11.0

- Breaking: the explicit page-break marker is `{pagebreak}` instead of `\pagebreak`. The backslash form is now ordinary text.
- The contract and technical-report fixtures use the new marker; regenerated their PDFs.

### 0.10.1

- The shared example template now sets a running header with `{page} de {pages}` in the right slot, clear of the letterhead.
- Added the `tecnico/` example, with its own template: running header and footer with every token, a styled table, a forced page break and document metadata.
- The contract fixture forces the signature block onto a new page with `\pagebreak`.
- Regenerated the example PDFs.

### 0.10.0

- Added `page.header` and `page.footer` with left, center and right slots. Slots expand the lowercase tokens `{page}`, `{pages}`, `{title}` and `{author}`; an unknown lowercase token is a template error, while an uppercase token stays literal for the secret phase.
- Added the `table` section with `border`, `border_color`, `header_fill` and `cell_padding`. The defaults keep the previous plain grid, and a zero border removes the stroke.
- Added the frontmatter `date`, `subject` and `keywords`. The date replaces the fixed document date and also sets the Typst world date, so the output stays deterministic; the subject and keywords reach the PDF metadata.
- A paragraph that contains only `\pagebreak` is an explicit page break, represented as `Block::PageBreak` in the document model.
- The template schema stays 0.5, because the new keys are optional and preserve the output of an existing template.
- The shared example template now styles tables, and both fixtures carry the new metadata. Regenerated both example PDFs.

### 0.9.1

- Adjusted the example template: leading 8, larger heading sizes, base heading spacing 36/12 and an `h1` spacing of 48.
- Changed the Contrato section headings to level 2.
- Regenerated both example PDFs.

### 0.9.0

- Breaking: template schema 0.5 uses a two-part string version such as `"0.5"` and replaces the `text` section with `formatting`.
- `formatting.paragraph` holds font families, size, weight, style, leading, spacing, alignment and the `emphasis` and `strong` inline overrides.
- `formatting.heading` is the base for `h1` to `h6`, each of which overrides font families, size, weight, style and spacing before and after.
- Added `weight` (normal, semibold, bold or 100 to 900) and `style` (normal or italic).
- Removed `text.body`, `text.title` and `text.header_spacing`.
- Regenerated both example PDFs.

### 0.8.0

- Breaking: template schema 4 makes `leading` a length in points, like the other spacing fields. A multiplier such as `0.65` is no longer accepted; use `7.8pt` for the same line spacing at 12pt.

### 0.7.0

- Breaking: template schema 3 replaces `text.font_families` and `text.size` with `text.body` and `text.title`, each with its own font families and size. Version 2 templates are rejected.
- A template length without a unit is now in points, and the example and default templates use points consistently.
- Added `text.header_spacing`, the space above a level-1 heading. When omitted, Typst's default heading spacing applies.
- Applied the title font and size to level-1 headings.
- Adjusted the example leading to 0.65.
- Regenerated both example PDFs with the new template.

### 0.6.0

- Added directory inputs: the command walks each directory recursively, processes `.md` files in path order, and skips hidden directories and `target/`.
- Added output tree mirroring under `--output` for directory inputs, with collision detection instead of silent overwrites.
- Added ANSI-colored, bold and italic terminal output with blank lines between results, and a `--color auto|always|never` option that respects `NO_COLOR` and `TERM=dumb`.
- Generalized atomic writes to any output extension.
- A failed file no longer stops the rest of the batch; the exit code is 1 when any input fails.
- Documented directory discovery, output mapping and terminal styling in `docs/architecture.md`, and updated the roadmap status.

### 0.5.0

- Added `text.paragraph_spacing`, a length that sets the space between paragraphs.
- Moved the example template to `assets/examples/.pageformer.yaml`, so it applies to both `Contrato.md` and `Embargos.md`, and regenerated both example PDFs with the shared fictional letterhead.
- Added `docs/roadmap.md`, the ordered implementation map from the CLI to 1.0.0.

### 0.4.0

- Breaking: removed the `document` section from the template. Titles, authors and languages now come only from the Markdown frontmatter, so one template serves many documents. Templates must use schema version 2.
- Legal numbering no longer needs a backslash: a loose ordered list whose items are one paragraph each becomes a sequence of numbered paragraphs, so `1.` stays as written and keeps its number. Tight ordered lists remain lists.
- Lists now carry their tightness, and the PDF passes it to Typst.
- Moved trailing commas outside the bold and italic spans in `Embargos.md`.
- Added the fictional `Timbre.png` letterhead and adjusted the example margins and leading.
- Purged an identifying location from the example fixture history and regenerated `Embargos.pdf` from the corrected Markdown.
- Disabled markdownlint's ordered-list prefix rule, because legal numbering is explicit and may restart.
- Revised the verification guide: every required tool installs without `sudo`, the Typst archive is extracted with Python when `tar -J` fails, and `qpdf`/`mupdf-tools` are optional.
- Documented why `tar -J` failed on machines where a user-level `xz` wrapper shadows `/usr/bin/xz`.

### 0.3.1

- Added a commented example template for the legal fixture.
- Regenerated both example PDFs with the current backend; both pass veraPDF 1.30.3 as PDF/A-2b.
- Added the `markdownlint-cli2` configuration and documented the verification tools and commands in `docs/verification.md`.

### 0.3.0

- Added the PDF backend: the document model and the resolved template become Typst source, which Typst compiles into PDF/A-2b.
- Escaped user text for Typst markup and user strings for Typst string literals.
- Loaded local images and the letterhead into the Typst project; remote images, missing files and unsupported formats are explicit errors.
- Added embedded and system font discovery, so a fallback font is always available.
- Made the PDF deterministic by fixing the document date.
- Added `--output` to choose the destination directory and `--check` to validate without writing; PDFs are written atomically next to the source by default.
- Rejected tables without columns in the document model.

### 0.2.0

- Added the library core with the backend-independent document model, the Markdown parser and the template loader.
- Defined schema version 1 of the YAML template: document defaults, page size, orientation, margins, letterhead, font families, text size, leading and alignment, all validated.
- Added the `pageformer` CLI that parses each Markdown file, resolves the applicable template and reports the result.
- Documented the template schema and the template resolution order in `docs/template.md`.
- Replaced the Typst spike and its dependencies with the real crate layout; the measurements remain the reference for the PDF backend.
- Added fixture tests for both examples, schema and resolution tests, CLI tests and property tests for parser totality.

### 0.1.0

- Documented the rendering pipeline, the format priorities and the secret-handling policy in `docs/architecture.md`.
- Chose Typst as the internal PDF engine and set `{NAME}` as the secret placeholder syntax.
- Added the Typst PDF/A-2b spike with a sample contract and recorded its binary size, build time and validation result.
- Added the example fixtures in `assets/examples/`: the contract Markdown and reference PDF, and the legal document extracted from an anonymized ODT.
- Recorded the Typst escaping constraints and the explicit PDF/A date requirement in `docs/architecture.md`.

## Revision History

*Patches, bug fixes, and minor updates.*

### 0.5.1

- Removed the stale `.examples/` entry from `.gitignore`.
