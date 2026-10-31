# Change Log

## Version History

*Major updates and feature releases.*

### 0.41.11

- Forced LF checkout for the committed HTML and TXT examples and marked the generated packages as binary in `.gitattributes`, so a Windows checkout keeps the example bytes identical to a fresh render.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.10

- Set `APPDATA` alongside `XDG_CONFIG_HOME` in the CLI confidential mapping tests, because Windows reads the user configuration directory from `%APPDATA%` and ignores `XDG_CONFIG_HOME`.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.9

- Escaped backslashes and quotes in the CLI confidential configuration fixture, so the folder mapping test writes valid YAML with Windows paths.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.8

- Escaped backslashes and quotes when the confidential folder tests write YAML fixtures, so Windows paths produce valid YAML and the mapping tests run on Windows.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.7

- Changed the Unicode task-marker PDF test to use the embedded Libertinus Serif and glyphs it covers (`✔` and `✘`), so the test no longer depends on DejaVu Sans being installed on the runner.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.6

- Made the confidential folder tests canonicalize their expected paths, so they pass on macOS, where `/var` resolves to `/private/var`.
- Filtered the expected `unknown font family` warnings in the PDF tests, because the templates list system fonts in preference order and a CI runner may not have every preferred family; every other warning still fails the test.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.5

- Added test-only GitHub Actions CI for Linux, macOS on Apple Silicon and Intel, and Windows, plus a gitleaks secret scan for the worktree and the full history.
- Added `docs/releasing.md` with the manual GitHub release process, the `v<version>` tag convention, the signed-tag check, the source archive and the checksum verification.
- Added the GitHub repository metadata to `Cargo.toml` and secret-file patterns to `.gitignore`.
- Fixed the two private intra-doc links in the DOCX and ODT module docs, so `cargo doc` is warning-free.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.4

- Revised the documentation as one set: the README now walks through the pipeline, the formats, the Markdown markers, the confidential fields and the publisher window, and includes a screenshot of `pageformer-publisher`.
- Rewrote `docs/architecture.md`, `docs/roadmap.md`, `docs/template.md` and `docs/verification.md` in a clearer structure, removed the historical spike notes and the completed phase work lists, and kept each document focused on one subject.
- Clarified the page, template and confidentiality rules, fixed the description of pagination, and added a regeneration summary to the verification guide.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.3

- Audited the Rust sources against `LANGUAGE.md` 1.0.0 and replaced behavior-selecting booleans with explicit enums: `ColorSupport`, `Terminal`, `ColorEnvironment`, `ItemSpacing`, `ItemPosition`, `CellKind`, `ParagraphPlacement`, `RunningParts`, `ShadingMode`, `EscapeMode` and `ExportAction`.
- Replaced numeric `as` casts with `TryFrom`/`From`, justified the three `too_many_arguments` allowances, removed avoidable clones in the asset map, the PDF paragraph fill and the publisher template row and status area, and made the HTML endnote lookups total with `get`/`get_mut`.
- No rendered output changes; the ODT examples are regenerated for the version in `meta:generator`.

### 0.41.2

- The `pageformer-publisher` title is now light gray instead of pure white, which softens it against the dark window.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.1

- Documented the deferred publisher action that would save a selected confidential INI into the folder map as a roadmap note in `docs/roadmap.md`; nothing is decided or implemented.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.41.0

- The confidential `files` map now accepts an explicit `.ini` file name as the key, so `"escritório particular.ini"` uses that exact file and `escritorio` keeps the implicit `escritorio.ini` default.
- INI names may contain spaces and Unicode; leading or trailing spaces, path separators, Windows-reserved characters and all-dot names are rejected. An explicit `.INI` suffix is accepted and preserves its case.
- Documented the key form in `docs/confidential.md` and added tests for spaces, explicit names and invalid characters.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.40.0

- Added the user-level confidential folder mapping. The public `config.yaml` lists confidential directories; each directory owns a `confidential.yaml` with a `files` map from an INI stem to its absolute source folder, so the INI is implicit and the template never knows it.
- The mapping resolves by the longest source-folder prefix. An explicit `--confidential-file` or the publisher field wins. The public configuration is read only from the user configuration directory, never from the project.
- The confidential directory must be `700`, and its map and INIs must be `600` on Unix. Missing directories, missing INIs, invalid names, relative paths, duplicate folders, unknown keys and unsupported versions are errors.
- `pageformer-publisher` uses the mapping when its confidential field is empty, and now reads the confidential environment variables when no file is selected.
- Documented the two-level configuration in `docs/confidential.md` and added CLI tests with a temporary `XDG_CONFIG_HOME`.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.39.0

- Added a title row to `pageformer-publisher`: `Pageformer Publisher` on the left in a larger face and the crate version as `v0.39.0` on the right in the small face.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.38.1

- Fixed the ODT image frame border: a uniform four-side border now uses `draw:stroke`, because LibreOffice ignored `fo:border-*` on the graphic style. A partial border still falls back to `fo:border-*`.
- Fixed the DOCX image frame: the grid and cell widths now include both side paddings, so the image no longer reaches the right border.
- Fixed the HTML and EPUB image frame: the requested width and the template maximum now belong to the `image-frame` span, and the image fills it with `width: 100%`. The box no longer expands to the full column or pushes the image to one side.
- Regenerates the examples, and the ODT examples carry the version in `meta:generator`.

### 0.38.0

- Added block images: a paragraph with only an image becomes `Block::Image`, with `-width:...-`, `-height:...-` and `-align:left|center|right-` markers on the adjacent lines. One dimension sets the other through the intrinsic ratio; values accept pixels, `px`, `mm`, `cm`, `in`, `pt` and percentages.
- Added `formatting.image` with `alignment` (default `center`), `max_width` (default `100%`), per-side `border` with padding, and `background`. The border and fill wrap the image instead of the full text column.
- The PDF, ODT, DOCX, HTML and EPUB backends render the image frame, alignment and size; TXT writes `[alt]`. The DOCX frame is a one-cell table, so its padding, border and fill stay around the image.
- Added `assets/examples/tecnico/Diagrama.png` and an image section to the technical report; the example templates now use different page backgrounds for testing.
- The document model now has `Block::Image`; `Document`, `Block`, `List`, `ListItem`, `Quote` and `FootnoteDefinition` no longer derive `Eq`, because image dimensions use floating point.
- Regenerates every example, and the ODT examples carry the version in `meta:generator`.

### 0.37.2

- Disabled the page background in the embedded default template: the key stays documented and commented, so the fallback paper remains white and the user opts in by uncommenting it.
- Aligned the embedded-template test with the disabled default and clarified the comment in `assets/default.pageformer.yaml`.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.37.1

- The embedded default template now declares `page.background: "#fdfdfd"` with a comment, so the fallback document has the page fill enabled and the key is visible as a starting point.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.37.0

- Added the optional `page.background` color: PDF fills the page, ODT fills the page layout, DOCX writes the document background with `displayBackgroundShape`, HTML paints the screen page and the printed paper, and EPUB paints the reflowable body. The letterhead stays above the fill.
- Reduced the monospace size in the reference and example templates to 0.875 of the body size, because a monospace face has a larger x-height and looked larger at the same point size. The embedded template uses `10.5pt` against `12pt`, and the technical report uses `9.5pt` against `11pt`.
- Documented the page background and the monospace size guidance, and clarified that the template margins describe the paged formats and HTML while EPUB leaves the text column to the reader.
- Regenerates the PDF, ODT, DOCX, HTML and EPUB examples; the ODT examples carry the version in `meta:generator`.

### 0.36.0

- Added the EPUB 3 backend and `--format epub`: chapters split at first-level headings, a generated navigation, a reflowable CSS subset, packaged body images and deterministic metadata.
- Added the `identifier` frontmatter field; without it, the EPUB identifier is a UUID v5 derived from the metadata and the rendered chapters.
- EPUB uses per-chapter endnotes, omits the letterhead and running content, and hides explicit page breaks because it is reflowable.
- `pageformer-publisher` EPUB button now exports, and every format button has a backend.
- Regenerates the EPUB, HTML and ODT examples; the ODT examples carry the version in `meta:generator`.

### 0.35.1

- Fixed empty HTML table cells collapsing: `th:empty::after` and `td:empty::after` add a non-breaking line box, so a signature grid keeps the row height of the other formats.
- Fixed the HTML list fill: the background now belongs to the first paragraph of each item, not to the whole `li`, so a nested list is not painted by its parent and the band follows the item indentation. The task marker stays inside the filled paragraph.
- Regenerates the HTML and ODT examples for the version in `meta:generator`.

### 0.35.0

- Added the self-contained HTML backend and `--format html`: one semantic HTML5 file with embedded CSS, data-URI images and no network request.
- The HTML follows the document model with headings, paragraphs, lists and tasks, tables with a `thead`, block quotes and alerts, code blocks with language classes, images with `alt`, and endnotes with `role="doc-endnotes"` and backlinks.
- The stylesheet derives page size, margins, fonts, leading, paragraph spacing, alignment, headings, rules, quote and code blocks, lists, tables and footnotes from the template. Print rules include page breaks, the letterhead as a page background, and the running header and footer as `@page` margin boxes with `counter(page)` and `counter(pages)`.
- Links allow relative destinations, `http`, `https` and `mailto`; an unsafe scheme becomes plain text with a warning and never reaches `href`.
- `html::stylesheet` exposes the generated CSS to the Pageweaver preview, and `Destination::scheme` centralizes the destination classification for later backends.
- The `pageformer-publisher` HTML button now exports instead of rendering as a placeholder. EPUB remains a placeholder.
- Regenerates the HTML examples and the ODT examples for the version in `meta:generator`.

### 0.34.1

- Added group comments to `assets/default.pageformer.yaml` explaining `page`, `formatting`, `paragraph`, `quote`, `code_block`, `list`, `heading`, `horizontal_rule` and `table`.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.34.0

- Fixed the PDF footnote: the marker now stays inline with the first footnote paragraph. The first paragraph is rendered without `#par` inside `#footnote`, so the marker no longer sits on its own line above the note.
- Expanded `assets/default.pageformer.yaml` to declare every key the schema accepts, with absent-by-design keys such as fills, rules and the letterhead shown commented. The embedded template is now a complete reference the user can copy and edit.
- Regenerates the technical report PDF and the ODT examples for the version in `meta:generator`.

### 0.33.0

- The `pageformer-publisher` format column now lists PDF, ODT, DOCX, HTML, EPUB and TXT in that order, colored red, green, blue, orange, yellow and gray. HTML and EPUB are disabled placeholders until their backends exist.
- The TXT button is split: the left half exports TXT and the `Copy` half puts the plain text on the clipboard. The copy never writes a file and never displays the text in the window.
- Added `pipeline::render` and `RenderOutcome`, so a backend can return bytes plus warnings without choosing an output path; `pipeline::export` now shares the same render path. The GUI uses `render` for the copy action.
- The drop box grows to match the six-button column, so the column is never taller than the drop area.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.32.4

- Softened the `pageformer-publisher` tooltip: the text is now dark gray (48) and the frame a less light gray (224), with a lighter border, so the hint no longer jumps against the dark window.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.32.3

- `pageformer-publisher` now embeds Liberation Sans Regular and registers it as the first proportional font, so every label, button, drop-box line and tooltip uses one regular-weight face on every platform. The system font probe is gone; the font is licensed under the SIL Open Font License 1.1, with the notice in `publisher/assets/fonts/OFL.txt`.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.32.2

- The `pageformer-publisher` tooltip text now uses a regular-weight system sans: Liberation Sans, DejaVu Sans, Noto Sans or FreeSans on Linux, Segoe UI or Arial on Windows, and Arial on macOS, with the embedded proportional font as the fallback. Black text on the light tooltip keeps crisp stems at 14 points.
- The minimum inner width of `pageformer-publisher` is now enforced programmatically as well as through the window hint, so the window snaps back to 520 points instead of letting the format buttons clip.
- The template path truncates with an ellipsis and keeps Edit and Folder visible; the drop box and the Advanced fields may shrink further before any button moves.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.32.1

- Renamed the Advanced template field to `Template:`.
- The confirmed `Selected:` line in `pageformer-publisher` is now baby blue and quieter than the surrounding text.
- Tooltips no longer take pointer input, so they never block the control under the cursor.
- The window height now follows the content, including the status footer and the Advanced body when it is open, so the scroll area is gone and no empty band remains below the content.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.32.0

- Added a template override to the Advanced section of `pageformer-publisher`, wired to `DocumentJob::with_template`; an empty field keeps automatic resolution. The Choose button accepts `.yaml` and `.yml`.
- Aligned the Advanced path rows (template, output and confidential) with a fixed label column and a uniform field and Choose width, and added right padding to the format column and the rows.
- Increased the tooltip text to 15 points, brightened the fill to near-white and kept black text, so the hint is legible at 1x.
- The window starts at 720x400 with a 520x260 minimum and scrolls when the content does not fit, instead of reserving empty space below.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.31.4

- Moved the Output folder into the Advanced section of `pageformer-publisher`, keeping the automatic fill from the selected Markdown file.
- Removed the manual Markdown path from Advanced: the drop box and the platform picker already cover file selection, so the typed path was redundant.
- Gave the `pageformer-publisher` tooltips a dedicated compact frame with a light-gray fill, black text, a thin border and its own shadow, instead of borrowing the window fill; the texts are shorter.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.31.3

- Moved the Confidential file into the Advanced section of `pageformer-publisher`, next to the manual Markdown path.
- Added explanatory tooltips to the labels, fields, buttons and status elements; the tooltip opens on the field name and on the field itself. The tooltips use a light-gray background with black text, and the `(?)` marker is gone.
- Added a Choose button to Output, matching Confidential; it picks a folder instead of a file.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.31.2

- Moved the confidential-output rule in `pageformer-publisher` to a tooltip on the Output label: "A document with confidential fields must be exported to a folder outside its source tree." The permanent hint line is gone, and the window is closer to the sketch.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.31.1

- Reworked the `pageformer-publisher` window to follow the sketch: the format buttons sit in a column on the right of a large drop box, and the template label keeps the edit and folder actions at the top.
- The drop box is clickable and opens a file chooser; on Linux it uses zenity, so it works on i3 without a desktop portal. Drag and drop remains available when the source supports XDND.
- The output directory is filled with the Markdown file's folder when a file is selected, so the default is visible instead of implicit.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.31.0

- Added `pageformer-publisher`, a workspace member with the minimal egui converter: drop or paste a Markdown path, choose PDF, ODT, DOCX or TXT, export and open the output. It shows the resolved template, an optional output directory and an optional confidential INI file.
- The window runs `pipeline::export` on a worker thread with `Idle`, `Working`, `Succeeded` and `Failed`; a second request is disabled while working. It never parses, resolves or writes itself.
- The workspace uses eframe with the glow and X11 features, so the Linux build does not need the Wayland development files. `cargo build -p pageformer` still builds only the engine and CLI.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.30.0

- Extracted the document pipeline into the library: `Format` in `src/format.rs`, and `DocumentJob`/`Source`, `check`/`export`, `CheckOutcome`/`ExportOutcome` and `ExportError` in `src/pipeline.rs`. The CLI is now a thin frontend, and Pageweaver can call the same API.
- The export accepts an in-memory Markdown buffer together with the source path used for template and asset resolution, so an unsaved editor buffer can be exported without a temporary file.
- Moved the atomic write into the pipeline; output errors are domain values and no `anyhow` crosses the library boundary.
- Named the minimal GUI `pageformer-publisher`; Pageweaver work is planned for after 1.0.0.
- Regenerates the ODT examples for the version in `meta:generator`.

### 0.29.1

- Added `docs/pageweaver.md`, describing the separate Pageweaver editor, the library export contract, the preview CSS boundary and the minimal converter GUI.
- Updated the README, the architecture page and the roadmap to name the minimal GUI and to record that Pageweaver is a separate Tauri project that exports through the Pageformer library, never by itself.

### 0.29.0

- Replaced the `{pagebreak}` marker with `-pagebreak-`, and added `-!-` as its shorthand. A paragraph that contains only one of them is a page break; the old brace form is now ordinary text. The change keeps braces reserved for confidential fields and running slots.
- Added `-horizontal-` as a word form of the thematic break, alongside the standard `---`, `***` and `___`.
- Renamed `formatting.rule` to `formatting.horizontal_rule`, so the key names the line it styles and no longer reads as a global rule. Bumped the template schema to 0.7; a template written for 0.6 is rejected instead of being read with the old key.
- Regenerates the examples for the new markers, the technical report prose and the ODT generator version.

### 0.28.1

- ODT footnote citations are now superscript both in the text and in the note body, using the `Footnote anchor` and `Footnote Symbol` character styles at `super 58%`. The PDF and DOCX already did this.
- Documented that a task `marker` accepts Unicode glyphs such as `☑`, `☐`, `✔` and `✗`, and that a monochrome symbol is the safe choice for PDF/A. The schema already carried the string; the tests now cover a Unicode pair.
- Regenerates the ODT examples for the generator version and the new footnote styles. The PDF, DOCX and TXT examples stay byte-identical.
- Documented that a hand-typed letter or roman label is ordinary text, not an ordered list; only numeric CommonMark markers form lists.

### 0.28.0

- Added `formatting.list.task`, with `done` and `todo` marker strings, text overrides for each state, a `gap` in non-breaking spaces and a `bullet` flag. With `bullet: false`, a task-only unordered list drops the list bullet and lets the checkbox be the visible marker; mixed and ordered lists keep their marker.
- The PDF renders the marker as inline `#text`/`#box` content, ODT uses `TaskDone`/`TaskTodo` character styles and `TaskList`, and DOCX uses the same character styles plus a marker-free numbering level with `w:numFmt` `none`. TXT keeps `[ ]` and `[x]` for copy-as-text.
- The default and technical templates declare green done and gray todo markers and hide the bullet on task-only lists. Regenerates the four formats; the PDFs stay deterministic, pass veraPDF as PDF/A-2b and keep their page counts.

### 0.27.1

- A nested list fill in the PDF now continues the parent item band instead of starting a new one, removing the hairline that remained between a parent item and its nested list. ODT and DOCX already shaded the run continuously.
- Regenerates the ODT examples for the version in `meta:generator` and the PDF example for the fill. The DOCX and TXT examples stay byte-identical.

### 0.27.0

- Added `formatting.list`, which styles list item text (face, foreground, background and alignment), the marker indent and gap, per-level bullet characters, per-level numbering formats with a suffix, and the spacing between loose items. Markers and formats cycle by nesting depth, so the template declares the order instead of relying on a backend default.
- The PDF maps the list style to `#set list` and `#set enum`, and a tight item fill merges with its neighbours the way the office paragraph shading does. ODT uses `ListItem` and `ListLooseItem` plus level data in `list_styles`; DOCX uses `ListParagraph` and the abstract numbering levels.
- Bumped the template schema to 0.6 and moved `table` under `formatting.table`, so every presentation section lives under `formatting`. A 0.5 template is rejected instead of being read with the old path.
- The technical example now has nested bullet and ordered lists, and both example templates declare their list style. Regenerates the four formats; the PDFs stay deterministic, pass veraPDF as PDF/A-2b and keep their page counts.

### 0.26.1

- Links are underlined only when the template declares `underline: true`; the default is now `false`, so the default and example templates state the underline explicitly and no backend supplies a hidden one.
- Regenerates the ODT examples for the version in `meta:generator`. The PDF, DOCX and TXT examples stay byte-identical.

### 0.26.0

- Added `formatting.paragraph.link`, so links take their face, foreground, background and underline from the template instead of a backend default. The map inherits the paragraph face, `underline` defaults to `true`, and the color stays declared-only; the default and both example templates declare a blue link.
- The PDF now shows the `[!KIND]` label of an alert block quote, the same way ODT and DOCX already did, so the three paged formats agree with the documented behavior and the technical fixture.
- A block fill in the PDF now adds the font-box padding at the top and bottom, the same 0.24 of the size used by heading fills, so a quote fill no longer hugs the text. A top or bottom border keeps its own larger padding.
- Regenerates the PDF, ODT, DOCX and TXT examples; the PDFs stay deterministic, pass veraPDF as PDF/A-2b and keep their page counts.

### 0.25.0

- Inline code and fenced code blocks now inherit the paragraph face when a key is undeclared, like every other inline style. There is no hidden monospace default left in the renderers, so a template can explain every font it sees.
- The default template and both example templates declare a monospace family for `code` and `code_block`, with a comment saying why the face differs.
- The technical example was expanded to demonstrate the inline formats, lists, task list, table, quote, alert, code block, thematic break, footnote and page breaks. It now renders four pages.
- Regenerates the office and technical examples; the PDFs stay deterministic and pass veraPDF.

### 0.24.0

- Added `alignment` to `formatting.quote` and `formatting.code_block`. Both default to `left`, because justified quotes and code read worse; `justified` follows the body again.
- Added `border` to `formatting.quote` and `formatting.code_block`, with `left`, `right`, `top` and `bottom` thicknesses, one `color` and a `padding` that applies to the bordered sides. A side is off when omitted, `none` or zero. The usual vertical quote rule is `border: { left: 3pt, color: "#cccccc", padding: 8 }`.
- Every inline style, including `code` and `strikethrough`, accepts `font_families`; emphasis, strong and strikethrough inherit the paragraph face, while code keeps its monospace default until a family is declared.
- Regenerates the office examples: blocks are left aligned now, and the ODT carries the version. The PDFs stay byte-identical and the page counts stay 4, 4 and 3.

### 0.23.0

- Added `background` to `formatting.paragraph`, to `emphasis` and `strong`, and to the new `formatting.quote` and `formatting.code_block` sections, so every text format now takes a `foreground` and a `background`.
- Added `formatting.paragraph.code` and `formatting.paragraph.strikethrough`, which style inline code and strikethrough with the same keys as `emphasis` and `strong`. They patch the renderer defaults, so a map with only `foreground` keeps the monospace face of code.
- Added `formatting.quote` and `formatting.code_block`, the first entries outside `paragraph`, with face, color and fill; both change nothing while undeclared.
- A body fill applies to body paragraphs only: quotes, lists and table cells keep the background their own style declares. Inline fills cover the span itself.
- Regenerated the ODT examples, which carry the version in `meta:generator`; page counts stay 4, 4 and 3.

### 0.22.0

- Every color field now accepts the 148 CSS named colors and the `#rgb` short form, besides `#rrggbb`, in any case: `foreground: navy`, `border_color: "#123"`. Names resolve to an RGB triple when the template loads, so no backend changed. `transparent` is rejected because the model has no alpha; `none` stays the way to clear an inherited value.
- The invalid-color error now names the accepted forms.

### 0.21.0

- Added `foreground`, a text color written as `#rrggbb`, to `formatting.paragraph`, to the `emphasis` and `strong` overrides and to `formatting.heading` and `h1` to `h6`. Headings inherit it from the base map and clear it with `none`, like `background` and `rule`; an inline span keeps the paragraph color unless it declares its own.
- Every format reads the color: Typst `fill`, `fo:color` in ODT and `w:color` in DOCX, including the running header and footer. TXT has no color.
- Documented the DOCX heading-fill divergence: a very large `formatting.paragraph.leading` makes the DOCX fill taller than the PDF and ODT ones.

### 0.20.2

- ODT headings now use the font's natural line height, `fo:line-height="100%"`, instead of the leading plus cap height. A heading fill therefore scales with the heading size, the same box DOCX gets from "at least" spacing: the ODT band measures 20.6pt at the 18pt heading and 16.3pt at 14pt, matching DOCX exactly. Body paragraphs keep the calibrated leading plus cap height.

### 0.20.1

- Raised the PDF heading fill padding from 0.15 to 0.24 of the heading size, so the PDF band matches the DOCX one, which is now the reference: at the 18pt heading the band grows from 17.8pt to 21.1pt against 20.6pt in DOCX, and at 14pt both measure 16.3pt.

### 0.20.0

- `background` and `rule` may now be declared on the base `formatting.heading` map, where every level inherits them, and a level clears an inherited value with `none`, in any case: `rule: none` or `background: NONE`. An empty `rule: {}` stays a rule with the defaults, which is not the same as `none`.
- The PDF heading fill gains a vertical padding of 0.15 of the heading size, so the bar no longer hugs the glyphs. At the 18pt level-one heading the band grows from 12.0pt to 17.8pt, next to the ODT 18.7pt and the DOCX 20.6pt.
- The technical example declares its rule once on the base heading map and clears it on `h1`, so the bar replaces it there. Its PDF, ODT and DOCX are regenerated at three pages, and the TXT golden is unchanged.

### 0.19.0

- Added `rule` to each heading level `h1` to `h6`: a line drawn below the heading text, styled with `thickness`, `color`, `spacing_before` (the gap above the line) and `spacing_after`. A rule is not inherited from the base heading map.
- The technical example draws a solid bar behind its level-one headings and a rule below its level-two headings. The regenerated PDF, ODT and DOCX keep three pages; the TXT golden and the shared examples are unchanged.
- The DOCX gap of a heading rule is clamped to the OOXML range of 0 to 31 points.

### 0.18.0

- Added the `formatting.rule` section to the template: `thickness`, `color`, `spacing_before` and `spacing_after` style the thematic break in every format. The defaults are `0.5pt`, `#000000` and each format's own spacing.
- Added `background` to `formatting.heading` and to `h1` to `h6`: a solid fill across the full text width with no horizontal margins, with the spacing kept outside the fill.
- The ODT and DOCX rules now follow the template. Their default thickness changes from `0.75pt` to `0.5pt` to match the PDF, and DOCX border widths are clamped to the OOXML range.
- Regenerated the ODT examples; their only change is the rule style, because the examples contain no thematic break.
- Added tests for the new keys, the PDF source, the ODT and DOCX parts and a PDF render with rules and backgrounds.

### 0.17.1

- Removed automatic hyphenation from the DOCX settings: LibreOffice asked for a dictionary even when the system package was installed. The document language stays in the styles, so the editor can enable hyphenation itself.
- Moved the DOCX header band up so the page number clears the letterhead rule, and switched line spacing to "at least" so a tight heading line no longer clips the top of the title.
- Table cell paragraphs now reset the paragraph spacing and the line, so rows no longer come out at double height.
- Regenerated the DOCX examples.

### 0.17.0

- Added `--format docx` and the DOCX backend: an Office Open XML package with the template's page layout, fonts, headings, lists, tables, letterhead, running header and footer, footnotes, hyperlinks, images and properties.
- Every list gets its own numbering instance with a start override; table header rows repeat; the template supplies borders, header fill and cell padding; the letterhead floats behind the header.
- The package is a stored ZIP assembled by the internal `src/zip.rs`; all relationship ids, numbering ids, drawing ids and property dates are fixed, so two runs are byte-identical.
- Shared the XML escaping, the Typst spacing model and the image size reader between the office backends in `src/xml.rs`, `src/metrics.rs` and `src/image.rs`.
- Added DOCX examples and structural tests, plus a LibreOffice smoke test documented in `docs/verification.md`.

### 0.16.2

- Enabled hyphenation with the frontmatter language on paragraphs, headings and cells, because Typst hyphenates and the ODT line breaks otherwise drift; the Embargos item 12 now stays on page 2, as in the PDF.
- The running header and footer use the paragraph face, so the page number is 12pt instead of a fixed 9pt, and reserve a 1.5cm band; the number now sits above the letterhead rule.
- Regenerated the ODT examples; the page counts remain 4, 4 and 3 and the page starts match the PDF.

### 0.16.1

- Matched the ODT spacing to the PDF: the ODF line height is the leading plus the cap height Typst uses, paragraph spacing subtracts the leading, heading margins account for the Writer line box, and tight list items carry no paragraph margin.
- Table cells no longer inherit paragraph spacing, and the leading is absorbed into the vertical padding, so rows have the same height as the PDF.
- A running header or footer reserves its band from the page margin, so the text area keeps the template margins and the header sits over the letterhead instead of pushing the body.
- The three ODT examples now match the PDF page counts (4, 4 and 3) and were regenerated.

### 0.16.0

- Added `--format odt` and the ODT backend: an OpenDocument text package with the template's page layout, fonts, headings, lists, tables, letterhead background, running header and footer, footnotes, links, images and metadata.
- The package is a stored ZIP with `mimetype` first and fixed timestamps, written by an internal `src/zip.rs`, so two runs are byte-identical and no compression dependency is added.
- Extracted the asset loader to `src/assets.rs`, shared by the PDF and ODT backends.
- Alerts become block quotes with a visible `[!KIND]` label, footnote definitions become inline notes, and confidential values never reach the metadata.
- Added ODT examples and structural tests with the `zip` and `roxmltree` dev-dependencies, plus a LibreOffice smoke test documented in `docs/verification.md`.

### 0.15.0

- Added `--format txt` and the plain-text backend: UTF-8 with LF line endings, no automatic wrapping, one blank line between blocks, Markdown-style list markers, tab-separated tables with a separator row, quotes prefixed with `>` and a space, `label (destination)` links, `[alt]` images, footnotes collected at the end and a form feed for an explicit page break.
- Introduced the renderer boundary: `Renderer` returns backend-neutral bytes plus warnings, `PdfRenderer` moved the PDF backend under it, and the TXT backend is total.
- Added text goldens for the office and technical examples, compared byte for byte, and regenerated them.
- Documented that the INI values file stays outside the Markdown and YAML sources, so editor `private_files` and `private_paths` rules can keep it out of agent file reads.

### 0.14.0

- Replaced the `NAME=value` confidential values file with an INI-style `NAME = value` format, which is easier to read and receives syntax highlighting in editors; the example is now `assets/examples/confidencial/embargos.ini`.
- Whitespace around the name, the `=` and the value is trimmed; `;` also starts a comment, alongside `#`; a value keeps every `=` after the first and any `#` or `;` that does not start the line; sections, quotes and escapes are rejected.
- The compact `NAME=value` form still parses, so existing files keep working.

### 0.13.2

- Shrank the development build: the dev profile keeps line tables only, dependencies carry no debug info and incremental compilation is off, so the main test binary dropped from about 540 MB to 155 MB and the debug tree from 88 GB to about 1.7 GB.
- Documented the build cache and its cleanup in the README.

### 0.13.1

- Moved the office examples to `assets/examples/escritorio/`, their fictional confidential values to `assets/examples/confidencial/embargos.env`, and the generated PDFs to `assets/examples/publicacao/`.
- Applied confidential fields to `Embargos.md` for the fictional company names, CNPJ, address, partner name, RG, CPF, court, process number, lawyers and case identifiers.
- Regenerated the office PDFs; both still pass veraPDF as PDF/A-2b.
- Documented that the committed values file is fictional and needs `chmod 600` after a checkout.

### 0.13.0

- Renamed the secrets phase to confidentiality and implemented it. A `{NAME}` in the body is a confidential field.
- Added `--confidential-file`, a `NAME=value` file outside the source tree with `0600` permissions on Unix; the file overrides the `PAGEFORMER_CONFIDENTIAL_<NAME>` environment variables.
- Added the document model resolution: text placeholders are substituted once, code spans and blocks, destinations, titles and frontmatter are excluded, and an unknown field is fatal.
- A document with confidential fields requires `--output` outside its source tree; `--check` lists names only.
- Known values are redacted from warnings and errors before stderr.
- A confidential field in frontmatter or in running header and footer content is now an error.
- Superscript and subscript are deferred until after 1.0.0; the roadmap records the decision.

### 0.12.1

- Added tests for every inline construct: reference links, angle autolinks, entities, escapes, single and double tilde strikethrough, and a rendering test with every inline variant.
- Documented the inline contract and its deliberate exclusions: math, superscript, subscript, highlight, wikilinks, inline HTML and smart punctuation.

### 0.12.0

- Added `page_break_before` to `formatting.heading` and to the `h1` to `h6` overrides. The break is weak, so a heading already at the top of a page does not create a blank page.
- Closed phase 2 of the roadmap. Typst already avoids widows and orphans, so no template field is exposed; PDF/A-2b remains the declared profile, since PDF/A-2u would only change the conformance identifier without a user need.
- The technical example starts each first-level heading on a new page; regenerated its PDF.

### 0.11.0

- Breaking: the explicit page-break marker is `{pagebreak}` instead of `\pagebreak`. The backslash form is now ordinary text.
- The contract and technical-report fixtures use the new marker; regenerated their PDFs.

### 0.10.1

- The shared example template now sets a running header with `{page} de {pages}` in the right slot, clear of the letterhead.
- Added the `tecnico/` example, with its own template: running header and footer with every token, a styled table, a forced page break and document metadata.
- The contract fixture forces the signature block onto a new page with `\pagebreak`.
- Regenerated the example PDFs.

### 0.10.0

- Added `page.header` and `page.footer` with left, center and right slots. Slots expand the lowercase tokens `{page}`, `{pages}`, `{title}` and `{author}`; an unknown lowercase token is a template error, while an uppercase token stays literal for the secret phase.
- Added the `table` section with `border`, `border_color`, `header_fill` and `cell_padding`. The defaults keep the previous plain grid, and a zero border removes the stroke.
- Added the frontmatter `date`, `subject` and `keywords`. The date replaces the fixed document date and also sets the Typst world date, so the output stays deterministic; the subject and keywords reach the PDF metadata.
- A paragraph that contains only `\pagebreak` is an explicit page break, represented as `Block::PageBreak` in the document model.
- The template schema stays 0.5, because the new keys are optional and preserve the output of an existing template.
- The shared example template now styles tables, and both fixtures carry the new metadata. Regenerated both example PDFs.

### 0.9.1

- Adjusted the example template: leading 8, larger heading sizes, base heading spacing 36/12 and an `h1` spacing of 48.
- Changed the Contrato section headings to level 2.
- Regenerated both example PDFs.

### 0.9.0

- Breaking: template schema 0.5 uses a two-part string version such as `"0.5"` and replaces the `text` section with `formatting`.
- `formatting.paragraph` holds font families, size, weight, style, leading, spacing, alignment and the `emphasis` and `strong` inline overrides.
- `formatting.heading` is the base for `h1` to `h6`, each of which overrides font families, size, weight, style and spacing before and after.
- Added `weight` (normal, semibold, bold or 100 to 900) and `style` (normal or italic).
- Removed `text.body`, `text.title` and `text.header_spacing`.
- Regenerated both example PDFs.

### 0.8.0

- Breaking: template schema 4 makes `leading` a length in points, like the other spacing fields. A multiplier such as `0.65` is no longer accepted; use `7.8pt` for the same line spacing at 12pt.

### 0.7.0

- Breaking: template schema 3 replaces `text.font_families` and `text.size` with `text.body` and `text.title`, each with its own font families and size. Version 2 templates are rejected.
- A template length without a unit is now in points, and the example and default templates use points consistently.
- Added `text.header_spacing`, the space above a level-1 heading. When omitted, Typst's default heading spacing applies.
- Applied the title font and size to level-1 headings.
- Adjusted the example leading to 0.65.
- Regenerated both example PDFs with the new template.

### 0.6.0

- Added directory inputs: the command walks each directory recursively, processes `.md` files in path order, and skips hidden directories and `target/`.
- Added output tree mirroring under `--output` for directory inputs, with collision detection instead of silent overwrites.
- Added ANSI-colored, bold and italic terminal output with blank lines between results, and a `--color auto|always|never` option that respects `NO_COLOR` and `TERM=dumb`.
- Generalized atomic writes to any output extension.
- A failed file no longer stops the rest of the batch; the exit code is 1 when any input fails.
- Documented directory discovery, output mapping and terminal styling in `docs/architecture.md`, and updated the roadmap status.

### 0.5.0

- Added `text.paragraph_spacing`, a length that sets the space between paragraphs.
- Moved the example template to `assets/examples/.pageformer.yaml`, so it applies to both `Contrato.md` and `Embargos.md`, and regenerated both example PDFs with the shared fictional letterhead.
- Added `docs/roadmap.md`, the ordered implementation map from the CLI to 1.0.0.

### 0.4.0

- Breaking: removed the `document` section from the template. Titles, authors and languages now come only from the Markdown frontmatter, so one template serves many documents. Templates must use schema version 2.
- Legal numbering no longer needs a backslash: a loose ordered list whose items are one paragraph each becomes a sequence of numbered paragraphs, so `1.` stays as written and keeps its number. Tight ordered lists remain lists.
- Lists now carry their tightness, and the PDF passes it to Typst.
- Moved trailing commas outside the bold and italic spans in `Embargos.md`.
- Added the fictional `Timbre.png` letterhead and adjusted the example margins and leading.
- Purged an identifying location from the example fixture history and regenerated `Embargos.pdf` from the corrected Markdown.
- Disabled markdownlint's ordered-list prefix rule, because legal numbering is explicit and may restart.
- Revised the verification guide: every required tool installs without `sudo`, the Typst archive is extracted with Python when `tar -J` fails, and `qpdf`/`mupdf-tools` are optional.
- Documented why `tar -J` failed on machines where a user-level `xz` wrapper shadows `/usr/bin/xz`.

### 0.3.1

- Added a commented example template for the legal fixture.
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

### 0.5.1

- Removed the stale `.examples/` entry from `.gitignore`.
