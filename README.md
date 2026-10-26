# Pageformer

*"One document, every format."*

Generates documents from Markdown content and a single generic YAML template. One template drives every output format, so all of them stay consistent.

## Stack

Rust with egui for the graphical interface. The main workflow runs in the terminal; the interface is a minimal window launched by argument.

## Versioning

SemVer; source: `Cargo.toml` (`[package].version`).

- Patch: fixes and minor refinements.
- Minor: new user-facing features or output formats.
- Major: breaking changes to the template format or the document model, or major release milestones.

## Goal

Take a Markdown file or folder and render it through a template into PDF, ODT and DOCX, with the same document model behind every backend. PDF is the primary output and ODT is the second priority; additional formats, such as HTML, plain text and EPUB, fit behind the same rendering trait.

Documentation is stored in [`docs/`](docs/). The rendering pipeline and the settled decisions are described in [`docs/architecture.md`](docs/architecture.md).
