# Verification

The project has four independent layers of verification: Rust checks, Markdown lint, PDF structure and PDF/A conformance. Every required tool installs at user level; `sudo` is only for the optional system packages.

## Installed tools

| Tool | Version | Purpose |
|---|---|---|
| `markdownlint-cli2` | 0.24.0 (markdownlint 0.42.0) | Markdown rules |
| `typst` | 0.15.1 | Standalone Typst compiler, matching the library version |
| `verapdf` | 1.30.3 | PDF/A conformance |
| `pdfinfo`, `pdffonts`, `pdftotext` | poppler-utils | PDF structure, fonts and text |

## Installation (no `sudo`)

Put the user binaries on `PATH` first:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Markdown lint. The `--prefix` keeps npm away from system directories:

```sh
npm install -g --prefix "$HOME/.local" markdownlint-cli2
```

Typst. The release is a `.tar.xz`; Python extracts it even when the local `xz` does not accept `tar -J`:

```sh
curl -L -o /tmp/typst.tar.xz https://github.com/typst/typst/releases/download/v0.15.1/typst-x86_64-unknown-linux-musl.tar.xz
mkdir -p /tmp/typst
python3 -m tarfile -e /tmp/typst.tar.xz /tmp/typst
install -m 755 /tmp/typst/typst-x86_64-unknown-linux-musl/typst "$HOME/.local/bin/typst"
```

Why not `tar -xJf`: on this machine `~/.local/bin/xz` is a custom wrapper that compresses a path into `<name>.tar.xz` and does not implement `-d`. Because `~/.local/bin` precedes `/usr/bin`, GNU tar's `-J` invokes the wrapper and fails with `Opção desconhecida: -d`. Python extraction bypasses the wrapper; alternatively, run `/usr/bin/xz -d` explicitly or put `/usr/bin` first for the command.

veraPDF. It needs Java 11 or newer (`java -version`). The installer is interactive: choose `$HOME/.local/verapdf` and select the **veraPDF CLI** pack. The greenfield directory name carries the version, so use a glob:

```sh
curl -L -o /tmp/verapdf-installer.zip https://software.verapdf.org/releases/verapdf-installer.zip
unzip -q /tmp/verapdf-installer.zip -d /tmp
/tmp/verapdf-greenfield-*/verapdf-install -console
ln -sf "$HOME/.local/verapdf/verapdf" "$HOME/.local/bin/verapdf"
```

The optional structure tools come from the distribution and are not needed for the checks below:

```sh
sudo apt-get install qpdf poppler-utils mupdf-tools
```

Poppler provides the `pdfinfo`, `pdffonts` and `pdftotext` used here; `qpdf --check` and `mutool info` add another structural opinion.

## Commands

Rust and the test suite:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc
cargo doc --no-deps
cargo build -p pageformer-publisher
cargo test -p pageformer-publisher
```

`cargo test --workspace --all-features` runs the engine, CLI and GUI tests. `cargo test --doc` runs the library doctests. `cargo doc --no-deps` documents the library; the GUI is a binary. `cargo build -p pageformer-publisher` builds the window, and `cargo test -p pageformer-publisher` covers the pure state machine and the path rules.

Markdown, using the repository `.markdownlint-cli2.jsonc`:

```sh
markdownlint-cli2
```

The configuration disables only rules that conflict with legal documents: line length, compact tables, explicit legal-numbering prefixes, multiple H1 section headings after a frontmatter title, and bold or italic lines used as headings. It ignores the session files and `target/`.

PDF/A conformance and structure:

```sh
verapdf --format text -f 2b assets/examples/publicacao/Contrato.pdf
verapdf --format text -f 2b assets/examples/publicacao/Embargos.pdf
verapdf --format text -f 2b assets/examples/tecnico/Relatorio.pdf

pdfinfo assets/examples/publicacao/Contrato.pdf
pdffonts assets/examples/publicacao/Embargos.pdf
pdftotext assets/examples/publicacao/Contrato.pdf -
```

A passing PDF/A run prints `PASS <path> 2b`. The tests also assert the PDF header, the PDF/A markers and that two renders produce identical bytes.

## Publisher GUI

`pageformer-publisher` is the minimal converter window in the `publisher/` workspace member. It calls `pipeline::export` on a worker thread and never parses, resolves or writes itself. The pure state machine, input acceptance, output paths and the source-root rule are covered by its unit tests.

The window itself is the only manual part of the suite: run `cargo run -p pageformer-publisher`, drop a Markdown file or click the drop box to choose one, choose a format, export and open the output. Advanced holds a template override, the output directory and the confidential file, all as aligned path rows with a Choose button; the output starts as the Markdown folder, and an empty template field keeps automatic resolution. The selected file appears in a quiet baby blue. Hovering a label or a field shows a compact light-gray tooltip with black text, a thin border and its own shadow; the text uses a regular-weight system sans, so it stays crisp at 14 points, and the embedded font is the fallback when no system font is found. The window height follows the content, including the status footer and the Advanced body when it is open, and the minimum inner width of 520 points is enforced programmatically, so the format and Choose buttons cannot be clipped by a narrower resize. The template path truncates and keeps Edit and Folder visible. On Linux the chooser is `zenity`, so it works on i3 without a desktop portal; on Windows and macOS it is the native `rfd` dialog. Drag and drop works from caja; zzzFM on i3 does not deliver XDND, also to Engrampa, so use the picker there. A windowed smoke test still needs a desktop session.

## ODT packages

`tests/odt.rs` unzips the package with the `zip` dev-dependency, parses every XML part with `roxmltree`, asserts that `mimetype` is first and stored, that the header fill, heading size, list markers, list formats, item styles, task marker styles and superscript footnote citations follow the template, and that two renders are byte-identical. The same tests assert that a confidential value never reaches `meta.xml`.

The LibreOffice smoke test converts the examples headlessly; the conversion must succeed without repair:

```sh
soffice --headless --convert-to pdf --outdir /tmp/odt-check assets/examples/publicacao/Contrato.odt assets/examples/publicacao/Embargos.odt assets/examples/tecnico/Relatorio.odt
```

The three examples match the PDF page counts (4, 4 and 4). Editor rendering can still break a line differently, so treat the count as a regression guard, not as a byte target.

## DOCX packages

`tests/docx.rs` unzips the package with the `zip` dev-dependency, parses every XML part with `roxmltree`, asserts that every required part and relationship exists, that every list has its own numbering instance with a restart and the template markers, formats and indents, that a task-only list can use the marker-free numbering, that the header row repeats, and that two renders are byte-identical. The same tests assert that a confidential value never reaches `docProps/core.xml`.

The LibreOffice smoke test converts the DOCX examples next to the ODT ones; the conversion must succeed without repair:

```sh
soffice --headless --convert-to pdf --outdir /tmp/office-check assets/examples/publicacao/Contrato.docx assets/examples/publicacao/Embargos.docx assets/examples/tecnico/Relatorio.docx
```

The DOCX target is a faithful package, not a page-count match: Word and LibreOffice lay out lines with their own engines.

## Plain-text goldens

The text goldens in `assets/examples/publicacao/` and `assets/examples/tecnico/` are compared byte for byte by `tests/txt.rs`. Regenerate them after a parser, model or text-renderer change and commit the result:

```sh
cargo run -- --format txt --confidential-file assets/examples/confidencial/embargos.ini --output assets/examples/publicacao assets/examples/escritorio/
cargo run -- --format txt assets/examples/tecnico/Relatorio.md
```

The `typst` CLI matches the library version used by the binary. The application compiles Typst in-process, so the CLI is a convenience for experimenting with Typst source, not a runtime dependency.
