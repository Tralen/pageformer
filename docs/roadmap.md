# Implementation Map

Pageformer reaches 1.0.0 through ten phases, in the order below. The order is a dependency chain, not a wish list: each phase removes an uncertainty that the next one would otherwise inherit. This map starts from the state at 0.5.0. The settled decisions live in [`architecture.md`](architecture.md), the schema in [`template.md`](template.md) and the verification tools in [`verification.md`](verification.md); this file only orders the work and states what closes each phase.

## Status at a glance

| Phase | Scope | Status | Starts after |
|---|---|---|---|
| 1. CLI | Input discovery, format selection, output rules | Done in 0.6.0: files and directories, path-ordered batches, mirrored output, ANSI styling | 0.5.0 |
| 2. PDF | Primary output and reference backend | Partial: PDF/A-2b, page and text, images, letterhead | CLI output rules |
| 3. Secrets | Placeholder resolution and leak prevention | Design only | PDF pipeline |
| 4. TXT | Plain-text view and the renderer boundary | Not started | Secrets |
| 5. ODT | Second-priority paged format | Not started | Renderer boundary |
| 6. DOCX | Office format for review and exchange | Not started | ODT style mapping |
| 7. GUI | Minimal egui window over the pipeline | Not started | Every format it exposes |
| 8. HTML | Self-contained semantic view | Not started | GUI and renderer boundary |
| 9. EPUB | HTML packaged with chapters and metadata | Not started | HTML |
| 10. Release 1.0.0 | Stability review and release | Not started | Every phase above |

```mermaid
flowchart TD
    CLI[1. CLI] --> PDF[2. PDF]
    PDF --> Secrets[3. Secrets]
    Secrets --> TXT[4. TXT and renderer boundary]
    TXT --> ODT[5. ODT]
    ODT --> DOCX[6. DOCX]
    DOCX --> GUI[7. GUI]
    GUI --> HTML[8. HTML]
    HTML --> EPUB[9. EPUB]
    EPUB --> Release[10. Release 1.0.0]
    Secrets -. placeholders resolved once .-> ODT
    Secrets -. placeholders resolved once .-> HTML
    TXT -. same backend contract .-> HTML
```

## Why this order

The CLI comes first because input discovery, format selection and output rules are the contract every backend sits behind. Changing them after five backends exist would touch every test and every usage example.

PDF comes second because it is the primary output and the reference for the document model. Every later backend copies decisions about headings, lists, tables, footnotes, images and metadata that the PDF backend forces us to make now.

Secrets come third, before the remaining backends. Placeholder resolution belongs to the pipeline, after parsing and before rendering. Implementing it after TXT, ODT or DOCX would mean writing the same scanner into several backends and then removing it.

TXT comes fourth because it is the cheapest backend and the first one that justifies a shared renderer boundary. It proves the trait with the least machinery, and it supplies the copy-as-plain-text action the GUI needs later.

ODT and DOCX come next because they are the office formats with the highest legal value and the closest semantic match to the PDF. They also force the shared style mapping, the deterministic ZIP packaging and the structural tests that HTML and EPUB will reuse.

The GUI comes after the formats it exposes, so its buttons map to a stable pipeline instead of to promises. HTML follows because a correct HTML view is the real prerequisite for EPUB; EPUB is HTML packaged with chapters and metadata.

Release 1.0.0 comes last and is not a feature. It is the point where the template schema and the document model are stable enough to promise compatibility.

## Contract for every phase

A phase is closed only when all of the following hold:

- The new behavior has a test that fails before the change and passes after it; the full Rust suite, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --doc` and `cargo doc --no-deps` pass.
- A backend receives a parsed document, a validated template and resolved assets, and returns bytes plus warnings. It never parses Markdown, reads a file, reads the environment or writes to disk.
- Expected failures are error values with domain variants. No `unwrap`, `expect`, `panic!` or unchecked indexing is added to library code.
- User-visible changes update `README.md`; settled decisions update `docs/architecture.md`; schema changes update `docs/template.md`; new validation tools update `docs/verification.md`.
- Output changes regenerate the examples and re-run their verification. Deterministic outputs stay byte-identical across two runs.
- `CHANGELOG.md` records the change, and the version follows the rules in `README.md`.

## Phase 1: CLI

**Goal.** Make one invocation the stable contract for every format: what is read, which format is produced, where it is written and what the exit code means.

**Current state.** Positional file arguments, `--template`, `--output`, `--check`, per-file diagnostics, atomic writes and `--version`. Each input is parsed and resolved independently.

**Status.** Done in 0.6.0: directory discovery, output planning, generalized atomic writes and ANSI-styled output.

**Work.**

- Accept directories as inputs, walk them recursively, collect `.md` files in lexicographic path order and skip hidden directories and `target/`. Do not follow directory symlinks, so a cycle cannot hang the run. A directory with no Markdown is a warning, not a failure.
- Introduce an internal `OutputFormat` enum now, with `Pdf` as the only exposed variant. Add `--format` when the second backend lands. The enum keeps the CLI free of stringly-typed branches later.
- Generalize `write_atomic`: it currently hardcodes `pdf.tmp`. Derive the temporary name from the final extension and keep the same-directory rename, so the write stays atomic for every format.
- Keep the current overwrite policy: an existing output is replaced atomically. Add a flag only if a real user need appears.
- Keep processing the batch after a failure, report one error per input on stderr, and exit with `1` when any input fails. `--check` keeps reporting success without writing.
- Keep help and error strings in English, stable enough to test, and never print document content.

**Done when.** `pageformer docs/` renders every Markdown file under the tree in a deterministic order, one bad file does not stop the others, both modes are covered by CLI tests that write only to temporary directories, and no test writes into `assets/examples/`.

## Phase 2: PDF

**Goal.** Turn the current backend into the reference implementation: the schema fields every paged format needs, with PDF/A conformance and deterministic bytes preserved.

**Current state.** PDF/A-2b through Typst 0.15.1, page size, orientation, margins, letterhead, font families, size, leading, paragraph spacing, alignment, local images, headings, lists, tables, quotes, code, footnotes, embedded fonts and a fixed document date.

**Work.**

- Add optional template fields with defaults that preserve the current output: `page.header` and `page.footer` with left, center and right slots; `text.heading` for size, weight, spacing before and after, and whether that level starts a new page; `table` for borders, header fill and cell padding.
- Define template tokens separately from document secrets. Lowercase tokens such as `{page}`, `{pages}`, `{title}` and `{author}` are expanded only in header and footer slots, and an unknown lowercase token is a template error. Uppercase `{NAME}` remains reserved for body secrets from phase 3.
- Settle the page-break syntax before coding it. A standalone `\pagebreak` paragraph is the leading candidate because CommonMark preserves it as text and the Zed preview shows it, while raw HTML stays rejected. Whatever the choice, it becomes a `Block::PageBreak` in the model, never a backend-specific string.
- Set the Typst text language from the frontmatter, pass image alternative text, keep table header rows and footnotes semantic, and let Typst build the outline from headings. Check that the tagged output survives these changes.
- Extend the frontmatter only when a backend needs the field. Candidate fields for this phase are `date`, which replaces the fixed date when present and must also drive the Typst world date so deterministic output holds, plus `subject` and `keywords`. Each new field is validated and documented in `docs/template.md`.
- Expose Typst's widow and orphan handling only through template fields that the examples actually use. Do not add knobs speculatively.
- Keep PDF/A-2b as the default. Decide whether to emit PDF/A-2u when every character maps to Unicode, as the architecture allows, and keep PDF/UA-1 out unless a user need appears.

**Done when.** Every new field has a schema test, a source-generation test and an example that uses it; both fixtures still pass veraPDF as PDF/A-2b; two runs produce identical bytes; the page-break and token rules are documented in `docs/architecture.md` and `docs/template.md`.

## Phase 3: Secrets

**Goal.** Resolve placeholders once, in the model, before any backend, and make a leak impossible by construction.

**Current state.** The design is settled in [`architecture.md`](architecture.md#secrets); no code exists.

**Work.**

- Add `SecretName`, an uppercase `[A-Z][A-Z0-9_]*` newtype, and `SecretValue`, a newtype whose `Debug` and `Display` never print the value. The value is built only by the loader, never from an arbitrary string.
- Scan text inlines only. Code spans, code blocks, link destinations, link titles, image destinations and frontmatter are excluded. A lowercase or spaced brace such as `{nome}` stays ordinary text.
- Treat an unknown placeholder as fatal. The error names the placeholder and its location, never a value, and never leaves the token in the output.
- Load values from the environment or from a file outside the source tree; accept no command-line flag for a value. Settle the file format as plain `KEY=VALUE` without shell evaluation, decide the file-over-environment precedence, and check `0600` permissions on Unix. Document the Windows behavior.
- Substitute after parsing and before asset loading, so every backend receives final text. The model gains a resolution function that returns a new document; the store never reaches a backend.
- Require `--output` when the document contains placeholders, and require that directory to resolve outside the canonical source tree. Refuse `.`, `..` tricks and a source tree that contains the output. `--check` lists placeholder names only.
- Redact known values from backend diagnostics before they reach stderr. Typst compilation errors can quote the generated source, which contains the substituted text, so redaction belongs at the error boundary.
- Test with dummy values injected through a temporary file and the environment: unknown placeholder, code span untouched, frontmatter rejection, output rule, `--check` names only, permission check, and an assertion that no dummy value appears in stdout, stderr or document metadata.

**Done when.** A secret-bearing document cannot be written next to its source, no output path can expose a value, and every later backend receives substituted text from the model without ever seeing the secret store.

## Phase 4: TXT

**Goal.** Prove the renderer boundary with the cheapest format and supply the copy-as-plain-text action the GUI will expose.

**Work.**

- Introduce the `Format` enum and the renderer contract, and move PDF under it. The pipeline resolves the document, template and assets, then calls one backend that returns bytes and warnings. Keep the trait small and backend-neutral.
- Extract asset resolution from `src/pdf/assets.rs` only as far as the new backends need it. The per-format policy stays explicit, for example PDF and ODT reject PDF images while HTML can embed them.
- Decide and document the plain-text rules: UTF-8 with LF, no automatic wrapping, one blank line between blocks, headings as their text, dash and ordered markers followed by a space (for example `-` and `1.`), nested indentation, tables as tab-separated cells with a separator row, quotes prefixed by a greater-than sign and a space, code blocks preserved, links as `label (destination)`, images as `[alt]`, footnotes collected at the end, soft breaks as spaces and hard breaks as newlines.
- TXT ignores layout fields, but the semantic template is still resolved, so a broken template fails the same way for every format.
- Add `--format txt` and the `.txt` output rule. The default output stays next to the source unless `--output` is given.
- Add golden text fixtures for both examples and focused cases for tables, lists, quotes and footnotes. Normalize line endings in the comparison, not in the output.

**Done when.** `--format txt` writes a readable file for both fixtures, the PDF output is unchanged by the new boundary, and the GUI can call the same pure function without a second conversion path.

## Phase 5: ODT

**Goal.** Deliver the second-priority format with the same semantic styles as the PDF.

**Work.**

- Choose the writer before coding. Prefer a small internal ODF writer, with a minimal ZIP dependency, over a full ODT library, unless a maintained crate covers styles, images, footnotes and deterministic packaging; record the decision in `docs/architecture.md`. Reject a crate that cannot express the template fields.
- Build the package with `mimetype` first and stored, then `META-INF/manifest.xml`, `content.xml`, `styles.xml` and `meta.xml`. Fix the entry order and timestamps.
- Map the template to ODF styles: page layout, font faces with a generic fallback, size, leading, paragraph spacing, alignment, headings, lists, tables, letterhead and footnotes.
- Map the document to ODF content: blocks, inlines, images under `Pictures/`, links and footnotes. Alerts become block quotes with a visible label, because ODF has no alert concept.
- Write title, author, language and generator to the metadata. Never write a secret value.
- Test the package structurally: unzip, parse every XML part, assert that `mimetype` is first and stored, and compare two renders byte for byte. Add an optional LibreOffice headless conversion as a smoke test and document the command in `docs/verification.md`.

**Done when.** Both fixtures open without repair, the text and structure match the model, a style change in the template is visible in the ODT, and two runs are byte-identical.

## Phase 6: DOCX

**Goal.** Produce the format reviewers expect, with lists, tables and footnotes intact.

**Work.**

- Choose between `docx-rs` and a hand-written OOXML writer, judged by determinism and coverage of the template fields. Reuse the semantic style resolution from ODT, never the ODF XML.
- Build the package: content types, relationships, `word/document.xml`, `word/styles.xml`, `word/numbering.xml`, `word/footnotes.xml`, media, and the core and app properties.
- Give each list its own numbering instance, so legal numbering restarts where the document says and tight and loose lists stay distinct.
- Repeat table header rows, apply borders and cell padding from the template, and take cell alignment from Markdown.
- Fix the core properties, the relationship ids, the numbering ids and the ZIP timestamps for determinism.
- Test the package structurally, smoke-test it in LibreOffice on Linux and in Word on Windows when available, and optionally round-trip it to text. DOCX has no conformance profile comparable to PDF/A.

**Done when.** Both fixtures open in Word and LibreOffice without repair, lists and footnotes survive the round trip, and two runs produce identical bytes.

## Phase 7: GUI

**Goal.** Expose the pipeline in a minimal egui window without duplicating any of it.

**Work.**

- Launch with `--gui`; the terminal remains the main workflow.
- Follow the sketch: a drop box, format buttons enabled only after a valid Markdown file, a template label with edit and open-folder actions, a status line and an open-output action.
- Model the window as a state machine with `Idle`, `Working`, `Succeeded` and `Failed`. Render on a worker thread and communicate through a channel, so the frame loop never blocks; a second request while working is disabled, not queued.
- Call the same pipeline. The GUI never parses Markdown, resolves a template or writes a file itself.
- Require an output directory outside the source tree when the document contains placeholders, and never display a secret value.
- Show errors in the status line and never panic on a bad file, a missing template or a failed render.
- Test the pure state transitions, input validation and button enablement. Keep the manual smoke test documented; use snapshot tests only if they stay deterministic.

**Done when.** A file can be dragged in and every implemented format generated, a template failure is visible without a crash, and the window stays responsive during a render.

## Phase 8: HTML

**Goal.** Produce a self-contained HTML view and the base for EPUB.

**Work.**

- Write one `.html` file with embedded CSS and data-URI images. The output makes no network request and is suitable for email.
- Use semantic HTML: headings, paragraphs, lists, tables with `thead`, block quotes, alerts, code with language classes, footnotes with `role="doc-endnotes"` and images with `alt`.
- Derive CSS from the template: page size and margins for print, fonts, size, leading, paragraph spacing, alignment, heading styles and table styling. Decide how the letterhead appears on screen and in print, and do not fake pagination.
- Escape all text and attributes. Link destinations may be remote and are emitted escaped, never fetched; images stay local and embedded. Restrict schemes to `http`, `https` and `mailto` and reject `javascript:` and `data:` at the boundary.
- Take title, author and language from the frontmatter.
- Fix the order of style rules and attributes for deterministic bytes.
- Compare the rendered HTML as normalized text for both fixtures, and add focused tests for escaping, tables and footnotes.

**Done when.** The file opens offline, print preview follows the template, and the block and inline conversion is reusable by the EPUB backend.

## Phase 9: EPUB

**Goal.** Package the HTML view as an EPUB 3 without a second converter.

**Work.**

- Split at each first-level heading; content before the first heading becomes the introduction; empty chapters are skipped. Build the navigation from the same headings.
- Reuse the HTML block and inline conversion, adding a chapter wrapper, a CSS subset and the metadata.
- Build the package with `mimetype` first and stored, then `META-INF/container.xml`, `content.opf`, `nav.xhtml`, chapters, CSS and images.
- Take title, language and author from the frontmatter. Settle the identifier: the architecture proposes a random UUID v4 per build, which conflicts with deterministic output; prefer a UUID v5 derived from a stable content hash unless the frontmatter supplies an identifier. Record the decision.
- Validate with `epubcheck` when installed, test the package structurally, and compare two renders byte for byte.

**Done when.** The EPUB passes `epubcheck`, chapters and navigation match the headings, images display offline, and two runs produce identical bytes.

## Phase 10: Release 1.0.0

**Goal.** Declare the document model and the template schema stable.

**Work.**

- Review stability: no pending breaking change to the template schema or the document model, and every promised format implemented and documented.
- Run the full verification matrix: the Rust checks, Markdown lint, veraPDF on both examples, DOCX, ODT and EPUB validation, a Linux smoke test and a Windows and macOS check.
- Set `1.0.0` in `Cargo.toml`, consolidate `CHANGELOG.md`, refresh the README usage and examples, and create a signed tag.
- State the compatibility promise for after 1.0.0: additive optional template keys are minor app releases and keep the current schema version; removing or changing a key is a major release and bumps it; a deprecated key keeps working for one minor cycle.

**Done when.** The full verification matrix passes on a clean checkout and the release notes state the compatibility promise.

## Template schema map

| Area | Keys | Phase | Notes |
|---|---|---|---|
| Version | `version` | 0.5.0 | Exact match; schema 2 is current. |
| Page | `page.size`, `page.orientation`, `page.margins`, `page.letterhead` | 0.5.0 | Shared by every paged format. |
| Text | `text.font_families`, `text.size`, `text.leading`, `text.paragraph_spacing`, `text.alignment` | 0.5.0 | Semantic, not backend-specific. |
| Header and footer | `page.header`, `page.footer` | 2 | Left, center and right slots; lowercase tokens. |
| Headings | `text.heading` | 2 | Size, weight, spacing, whether the level starts a new page. |
| Tables | `table` | 2 | Borders, header fill, cell padding. |
| EPUB | `epub.split_at` | After 1.0.0 | Defaults to first-level headings. |

Schema evolution follows one rule: an optional key that preserves the meaning and the output of existing templates keeps the current schema version; removing a key or changing the meaning of an existing one bumps the schema version and is a breaking change. A newer template with an unknown key is rejected by an older build, which is the intended failure.

## Frontmatter map

| Field | Phase | Purpose |
|---|---|---|
| `title`, `author`, `language` | 0.5.0 | Document metadata; required by PDF, ODT, DOCX, HTML and EPUB. |
| `date` | 2 | Document date; replaces the fixed date when present. |
| `subject`, `keywords` | 2 | PDF metadata and HTML meta tags. |
| `identifier` | 9 | Stable EPUB identifier; overrides the derived UUID. |

## Risks to watch

- **Repeated escaping.** Each output language needs its own escaper: Typst markup and strings, XML, HTML and JSON in EPUB metadata. Share nothing between them except the tests and the rule that user text is never trusted.
- **Broken determinism.** ZIP timestamps, random identifiers, map iteration order and current time are the usual causes. Fix the first three as soon as the first ZIP format lands, and assert byte equality in every output test.
- **Secret leaks through diagnostics.** Compilers quote source text. Redact known values at the error boundary, not only in the happy path.
- **Unsafe link schemes.** The parser accepts any link destination today. HTML must reject `javascript:` and `data:`, and the shared model should validate schemes once so no backend can emit a dangerous target.
- **Blocking the GUI.** A render takes long enough to freeze a frame loop. Keep it on a worker thread from the first version.
- **Schema drift.** Adding fields without updating `docs/template.md` and the tests makes the version meaningless. Treat the schema map above as part of the phase contract.
- **Premature abstraction.** Do not add a backend-specific hook to the renderer before two backends need it, and do not start the GUI before the formats it exposes exist.
