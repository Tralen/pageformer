# Architecture

Pageformer turns a Markdown document plus one YAML template into several output formats. The pipeline is shared; only the last step changes.

```mermaid
flowchart TD
    MD[Markdown and frontmatter] --> Parser[CommonMark parser]
    Parser --> AST[Project document model]
    YAML[pageformer.yaml] --> Resolver[Template resolution and validation]
    Resolver --> Template[Typed template]
    AST --> Render[Render model]
    Template --> Render
    Secrets[Secret values] --> Render
    Render --> PDF[PDF]
    Render --> ODT[ODT]
    Render --> DOCX[DOCX]
    Render --> HTML[HTML]
    HTML --> EPUB[EPUB]
    Render --> TXT[Plain text]
```

## Document model

Markdown is parsed into a project-owned document model, not into backend-specific structures. The model holds semantic blocks and inlines, such as headings, paragraphs, lists, tables, quotes, code, images and explicit page breaks, and carries no layout decisions. Backends read the model and never parse Markdown themselves.

The template is parsed and validated separately into a typed structure. An invalid template is fatal: the renderer never falls back to another template, because a document generated with the wrong template is worse than no document.

The Markdown subset is CommonMark plus the extensions the Zed preview renders: tables, task lists, strikethrough, footnotes, alert block quotes and YAML frontmatter. Fenced code blocks are preserved. Raw HTML is not part of the contract.

## Consistency

Every format shares the semantic template: page size, orientation, margins, fonts, colors, line spacing, heading styles, header and footer content and the letterhead image.

The promise is consistency of presentation, not identical content distribution. Pagination differs between paged formats, such as PDF, and reflowable formats, such as ODT, DOCX and EPUB. The project does not try to force the same distribution. Widows and orphans must be avoided where the engine allows it.

## Template resolution

The nearest `.pageformer.yaml` wins entirely. There is no merge across directories: a local template replaces the global template and the embedded fallback. If the winning template is invalid, generation fails with a diagnostic that identifies the file, line and column.

The global template lives in the user configuration directory. A minimal fallback is embedded in the binary.

Paths inside the template, such as the letterhead image or a font file, resolve relative to the template file, not to the working directory.

## Output

Without secrets, the output is written next to the Markdown source, with the same name and the new extension. With secrets, the output must be written to a directory outside the source tree, given explicitly; the program refuses to write a secret-bearing document next to its sources. Writes are atomic.

## Secrets

Placeholders use the form `{NAME}` in the document body, with an uppercase name such as `{NOME_COMPLETO}`. Single braces are literal text in CommonMark, so both the parser and the Zed preview keep them intact. The angle-bracket form is rejected because `<NAME>` is parsed as inline HTML.

Every token that matches `{[A-Z][A-Z0-9_]*}` is reserved for a secret. An unknown placeholder is a fatal error, never text left in the final document. The pattern does not apply inside code spans, code blocks or URLs, where the text is reproduced literally. A lowercase or spaced brace, such as `{nome}`, is ordinary text.

Placeholders are resolved only in body content. One found in frontmatter metadata is an error, because secret values must never reach the PDF XMP package, the DOCX core properties or the ODT metadata.

Secret values are loaded from a file outside the source tree, or from the environment, and are never accepted as command-line arguments. They are redacted from debug output and error messages and never printed to the console. A `--check` run reports placeholder names only.

Metadata comes from the template and the frontmatter only. The document model replaces placeholders at render time, after parsing and before a backend sees the content.

## Formats

- PDF is the primary output and must conform to PDF/A-2b, or PDF/A-2u when every character maps to Unicode.
- ODT is the second priority.
- DOCX follows.
- HTML, plain text and EPUB reuse the same document model. HTML is the basis for EPUB, which splits chapters at first-level headings and uses deterministic metadata.
- Plain text is a view of the model, used by the copy-as-plain-text action.

## PDF engine

The engine is Typst, used as a library. The constraint is a self-contained binary, with no Chromium, no webview and no external converter, and Typst satisfies it while providing the layout work the project would otherwise have to implement.

Typst provides pagination, headers and footers, page numbering, explicit page breaks, automatic avoidance of single-line widows and orphans, system font discovery, font subsetting and embedding, PDF/A-2b, PDF/A-2u and PDF/UA-1. The Typst language stays internal: the YAML template and the document model generate Typst content, and users never write Typst. A small `World` implementation for fonts and file access is preferred over a wrapper with an unstable API.

The crate is Apache-2.0, compatible with the project license.

### Spike results

The first spike (Typst 0.15.1, Rust 1.99.1, release profile with thin LTO and stripped symbols) rendered a nine-page A4 contract with system fonts, a header, a footer with page numbers, a table and signature lines.

| Measurement | Result |
|---|---|
| Binary size | 40,387,192 bytes (about 38.5 MiB) |
| Clean build | 62.6 s, peak RSS about 1.9 GiB |
| Runtime | about 0.17 s, peak RSS about 33 MiB |
| Output | 73,781 bytes, PDF 1.7, nine A4 pages, tagged |
| Fonts | Liberation Serif regular, bold and italic, embedded and subsetted |
| PDF/A-2b | PASS with veraPDF 1.31.207 |

The document date is fixed to 2026-01-01 UTC, so the output is deterministic: two runs and a clean rebuild produced the same SHA-256. The sample source is `assets/spike.typ`; the temporary entry point is `src/main.rs`.

krilla remains the fallback. It is a high-level PDF writer with PDF/A and PDF/UA support, but it explicitly leaves text layout, tables, page breaking and headers to the caller; choosing it would mean implementing those algorithms in the project.

PDF/A output is validated with veraPDF, because producing a valid PDF/A file is not the same as conforming to the profile.

## Testing

Pure tests cover template resolution, validation, placeholder handling and the document model. HTML and plain text are compared as text. ZIP-based formats are checked structurally. PDF/A output is validated with veraPDF in integration tests. A corpus of real documents is kept as fixtures, because typography regressions do not appear in synthetic samples.

The first fixtures are in `assets/examples/`: `Contrato.md` with the matching reference `Contrato.pdf`, and `Embargos.md`, extracted from an anonymized ODT. The reference PDF is for visual comparison, not byte equality.

## Open decisions

- Exact YAML schema and its version field.
- ODT implementation: a native ODF writer or another path.
- DOCX library choice and the shared document model for reflowable formats.
- Secret file format and permission checks.
- Crate layout for the pure core and the binary.
