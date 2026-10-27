# Pageformer

*"One document, every format."*

Generates documents from Markdown content and a single generic YAML template. One template drives every output format, so all of them stay consistent.

## Stack

Rust with egui for the graphical interface. The main workflow runs in the terminal; the interface is a minimal window launched by argument. PDF rendering uses Typst as an internal library, and users never write Typst.

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
cargo run -- assets/examples/Contrato.md assets/examples/Embargos.md
cargo run -- assets/examples/
cargo run -- --check assets/examples/
cargo run -- --output /tmp/pageformer assets/examples/
cargo run -- --check --color=never assets/examples/Contrato.md
```

The first command writes each PDF next to its Markdown and updates the versioned examples in `assets/examples/`. A file or a directory can be given; a directory is walked recursively and only `.md` files are read. Hidden directories and `target/` are skipped, and directory symlinks are not followed.

`--output` writes under another directory; a directory input mirrors its relative tree there, while a direct file is written flat. `--check` parses the Markdown and validates the template without writing. A failure in one file does not stop the rest of the batch, and the exit code is `1` when any input fails.

The terminal output uses color, bold and italic when the stream is a terminal. `--color=auto` is the default and respects `NO_COLOR` and `TERM=dumb`; use `--color=always` or `--color=never` to force or disable it.

## Examples

The fixtures in [`assets/examples/`](assets/examples/) exercise the parser and the PDF backend. `Contrato.md` has a generated `Contrato.pdf`; `Embargos.md`, extracted from an anonymized ODT, has `Embargos.pdf`. Both PDFs are generated with the shared `.pageformer.yaml` in that directory, which includes the fictional `Timbre.png` letterhead, and both pass veraPDF as PDF/A-2b.
