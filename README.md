# Pageformer

*"One document, every format."*

Generates documents from Markdown content and a single generic YAML template. One template drives every output format, so all of them stay consistent.

## Stack

Rust with egui for the graphical interface. The main workflow runs in the terminal; the interface is a minimal window launched by argument. PDF rendering uses Typst as an internal library, and users never write Typst.

## Versioning

SemVer; source: `Cargo.toml` (`[package].version`).

- Patch: fixes and minor refinements.
- Minor: new user-facing features or output formats.
- Major: breaking changes to the template format or the document model, or major release milestones.

## Goal

Take a Markdown file or folder and render it through a template into PDF, ODT and DOCX, with the same document model behind every backend. PDF is the primary output and ODT is the second priority; additional formats, such as HTML, plain text and EPUB, fit behind the same rendering trait.

Documentation is stored in [`docs/`](docs/). The rendering pipeline and the settled decisions are described in [`docs/architecture.md`](docs/architecture.md), the YAML schema in [`docs/template.md`](docs/template.md), and the verification tools in [`docs/verification.md`](docs/verification.md).

## Usage

The command renders each Markdown file to PDF/A next to its source, using the template resolved for it.

```sh
cargo run -- assets/examples/Contrato.md
cargo run -- --check assets/examples/Contrato.md
cargo run -- --output /tmp/pageformer assets/examples/Contrato.md
```

The first command writes `Contrato.pdf` next to the Markdown and replaces the reference file in `assets/examples/`. Use `--output` to write somewhere else and `--check` to parse the Markdown and validate the template without rendering.

## Examples

The fixtures in [`assets/examples/`](assets/examples/) exercise the parser and the PDF backend. `Contrato.md` has a generated `Contrato.pdf`; `Embargos.md`, extracted from an anonymized ODT, has `Embargos.pdf` generated with the commented example template `Embargos.pageformer.yaml`. Both PDFs pass veraPDF as PDF/A-2b.
