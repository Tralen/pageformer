# Change Log

## Version History
*Major updates and feature releases.*

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
