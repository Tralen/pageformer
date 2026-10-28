# Pageformer

*"One document, every format."*

Generates documents from Markdown content and a single generic YAML template. One template drives every output format, so all of them stay consistent.

## Stack

Rust with egui for the graphical interface. The main workflow runs in the terminal; the interface is a minimal window launched by argument. PDF rendering uses Typst as an internal library, and users never write Typst.

## Build

The development profile keeps line tables for this crate and no debug info for dependencies, because every test binary links Typst and would otherwise weigh hundreds of megabytes; incremental compilation is off because its state reached several gigabytes for little gain in a crate this small. `cargo clean` reclaims the cache when it grows, at the cost of a full rebuild.

## Versioning

SemVer; source: `Cargo.toml` (`[package].version`).

- Patch: fixes and minor refinements.
- Minor: new user-facing features or output formats. While the project is 0.x, breaking changes to the template format or the document model also use the minor field.
- Major: breaking changes to the template format or the document model after 1.0.0, or major release milestones.

## Goal

Take a Markdown file or folder and render it through a template into PDF, ODT and DOCX, with the same document model behind every backend. PDF is the primary output and ODT is the second priority; additional formats, such as HTML, plain text and EPUB, fit behind the same rendering trait.

Documentation is stored in [`docs/`](docs/). The rendering pipeline and the settled decisions are described in [`docs/architecture.md`](docs/architecture.md), the YAML schema in [`docs/template.md`](docs/template.md), the verification tools in [`docs/verification.md`](docs/verification.md), and the ordered path to 1.0.0 in [`docs/roadmap.md`](docs/roadmap.md).

## Usage

The command renders each Markdown file to PDF/A next to its source, using the template resolved for it.

```sh
cargo run -- --confidential-file assets/examples/confidencial/embargos.env --output assets/examples/publicacao assets/examples/escritorio/
cargo run -- --check --confidential-file assets/examples/confidencial/embargos.env assets/examples/escritorio/
cargo run -- assets/examples/tecnico/Relatorio.md
cargo run -- --output /tmp/pageformer --confidential-file assets/examples/confidencial/embargos.env assets/examples/escritorio/
cargo run -- --check --color=never --confidential-file assets/examples/confidencial/embargos.env assets/examples/escritorio/Contrato.md
```

The first command writes each PDF under `assets/examples/publicacao/`, which keeps it outside the source tree, and regenerates the versioned examples. A file or a directory can be given; a directory is walked recursively and only `.md` files are read. Hidden directories and `target/` are skipped, and directory symlinks are not followed. A paragraph that contains only `{pagebreak}` starts a new page.

`--output` writes under another directory; a directory input mirrors its relative tree there, while a direct file is written flat. `--check` parses the Markdown and validates the template without writing. A failure in one file does not stop the rest of the batch, and the exit code is `1` when any input fails.

A `{NAME}` field in the body is a confidential field. Its value comes from a `PAGEFORMER_CONFIDENTIAL_NAME` environment variable or from a `NAME=value` file passed with `--confidential-file`; the file must sit outside the source tree and, on Unix, be readable only by its owner. A document with fields must be written with `--output` to a directory outside its source tree. `--check` lists the field names without their values, and unknown fields are errors. The committed `assets/examples/confidencial/embargos.env` holds fictional values and must be `chmod 600` after a checkout, because Git does not preserve that mode.

The terminal output uses color, bold and italic when the stream is a terminal. `--color=auto` is the default and respects `NO_COLOR` and `TERM=dumb`; use `--color=always` or `--color=never` to force or disable it.

## Examples

The fixtures in [`assets/examples/`](assets/examples/) exercise the parser and the PDF backend. `escritorio/` holds `Contrato.md` and `Embargos.md` with the shared `.pageformer.yaml`, its fictional `Timbre.png` letterhead and a page-number header. `confidencial/embargos.env` holds the fictional values for the fields in `Embargos.md`, and `publicacao/` holds the generated `Contrato.pdf` and `Embargos.pdf`; the signature block of the contract is on its own page. The nested `tecnico/Relatorio.md` has its own template, which demonstrates the running header and footer, the table style, an explicit page break and first-level headings that start a new page, without a letterhead. All three PDFs pass veraPDF as PDF/A-2b.
