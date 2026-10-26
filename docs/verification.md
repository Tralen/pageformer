# Verification

The project has four independent layers of verification: Rust checks, Markdown lint, PDF structure and PDF/A conformance. The tools below are installed at user level where possible, so only the optional system packages need `sudo`.

## Installed tools

| Tool | Version | Purpose |
|---|---|---|
| `markdownlint-cli2` | 0.24.0 (markdownlint 0.42.0) | Markdown rules |
| `typst` | 0.15.1 | Standalone Typst compiler, matching the library version |
| `verapdf` | 1.30.3 | PDF/A conformance |
| `pdfinfo`, `pdffonts`, `pdftotext` | poppler-utils | PDF structure, fonts and text |

## Installation

`markdownlint-cli2`, `typst` and `verapdf` do not need `sudo`.

```sh
npm install -g markdownlint-cli2

curl -L -o /tmp/typst.tar.xz https://github.com/typst/typst/releases/download/v0.15.1/typst-x86_64-unknown-linux-musl.tar.xz
tar -xJf /tmp/typst.tar.xz -C /tmp
install -m 755 /tmp/typst-x86_64-unknown-linux-musl/typst ~/.local/bin/typst

curl -L -o /tmp/verapdf-installer.zip https://software.verapdf.org/releases/verapdf-installer.zip
unzip -q /tmp/verapdf-installer.zip -d /tmp
/tmp/verapdf-greenfield-*/verapdf-install -console
# Choose ~/.local/verapdf and select the "veraPDF CLI" pack.
ln -s ~/.local/verapdf/verapdf ~/.local/bin/verapdf
```

The optional PDF structure tools come from the distribution:

```sh
sudo apt-get install qpdf poppler-utils mupdf-tools
```

Poppler is already enough for the commands below; `qpdf --check` and `mutool info` add another structural opinion.

## Commands

Rust and the test suite:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --doc
cargo doc --no-deps
```

Markdown, using the repository `.markdownlint-cli2.jsonc`:

```sh
markdownlint-cli2
```

The configuration disables only rules that conflict with legal documents: line length, compact tables, multiple H1 section headings after a frontmatter title, and bold or italic lines used as headings. It ignores the session files and `target/`.

PDF/A conformance and structure:

```sh
verapdf --format text -f 2b assets/examples/Contrato.pdf
verapdf --format text -f 2b assets/examples/Embargos.pdf

pdfinfo assets/examples/Contrato.pdf
pdffonts assets/examples/Embargos.pdf
pdftotext assets/examples/Contrato.pdf -
```

A passing PDF/A run prints `PASS <path> 2b`. The tests also assert the PDF header, the PDF/A markers and that two renders produce identical bytes.

The `typst` CLI matches the library version used by the binary. The application compiles Typst in-process, so the CLI is a convenience for experimenting with Typst source, not a runtime dependency.
