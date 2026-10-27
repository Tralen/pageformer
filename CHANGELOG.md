# Change Log

## Version History

*Major updates and feature releases.*

### 0.4.0

- Renumbered the unreleased 1.0.0 and 1.1.0 changes under 0.4.0; no release was made.
- Breaking: removed the `document` section from the template. Titles, authors and languages now come only from the Markdown frontmatter, so one template serves many documents. Templates must use schema version 2.
- Legal numbering no longer needs a backslash: a loose ordered list whose items are one paragraph each becomes a sequence of numbered paragraphs, so `1.` stays as written and keeps its number. Tight ordered lists remain lists.
- Lists now carry their tightness, and the PDF passes it to Typst.
- Moved trailing commas outside the bold and italic spans in `Embargos.md`.
- Added the fictional `Timbre.png` letterhead to the `Embargos` example and adjusted its margins and leading.
- Purged an identifying location from the example fixture history and regenerated `Embargos.pdf` from the corrected Markdown.
- Disabled markdownlint's ordered-list prefix rule, because legal numbering is explicit and may restart.
- Revised the verification guide: every required tool installs without `sudo`, the Typst archive is extracted with Python when `tar -J` fails, and `qpdf`/`mupdf-tools` are optional.
- Documented why `tar -J` failed on machines where a user-level `xz` wrapper shadows `/usr/bin/xz`.

### 0.3.1

- Added `assets/examples/Embargos.pageformer.yaml`, a commented example of the template DSL.
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
