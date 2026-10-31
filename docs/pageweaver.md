# Pageweaver

Pageweaver is the separate WYSIWYG editor for Pageformer documents. It edits the Markdown source continuously, displays the document with the styles resolved from the Pageformer template, and asks Pageformer to export the final files. This page records the boundary between the two projects.

## Roles

- **Pageformer** is the engine and the CLI: Markdown parser, document model, template schema and resolution, style resolution, confidential fields, assets, renderers and atomic writes. It is the source of truth for the final appearance.
- **Pageweaver** is the authoring app: continuous editing, notebook navigation, outline, local links and images, and interface themes. It does not parse Markdown, resolve templates or render final files itself.
- **The minimal GUI** in Pageformer is `pageformer-publisher`, a converter for people who only need to export a document. It has no editing, notebook or live preview.

## Status

Pageweaver work starts only after Pageformer reaches 1.0.0. Until then the Pageformer library, the CLI and `pageformer-publisher` are the priority; the editor is planned but not started.

## Why two projects

The editor uses a different stack (Tauri, Tiptap/Milkdown, a web view) and has a different release cadence. Keeping it separate keeps the engine free of editor dependencies and the CLI lean. The contract between them is the library API, not the CLI.

## Export contract

Pageweaver calls the Pageformer library in process. The export accepts the Markdown buffer and the source path, so an unsaved buffer can be exported without a temporary file; the source path resolves the template, the letterhead and the images. Pageformer owns the parse, template resolution, confidential substitution, rendering and atomic write. Pageweaver owns the editor state, the format and destination choice, and the error presentation.

The export returns a domain `ExportOutcome` and a domain `ExportError`; `anyhow` stays at the binary boundary. The public types are `Format`, `DocumentJob`, `Source`, `CheckOutcome`, `ExportOutcome`, `RenderOutcome`, `ResolvedTemplate` and `ExportError`, and the entry points are `resolve_template`, `check`, `render` and `export` in `src/pipeline.rs`. `render` returns bytes plus warnings without choosing an output path, which is what the preview and the copy action need; `export` writes the file. The caller runs the export on a worker thread, so the UI stays responsive. The supported formats come from the library; today PDF, ODT, DOCX, HTML, EPUB and TXT.

Confidential fields stay opaque in the editor: `{NAME}` is shown as a placeholder, never resolved, and Pageformer never puts a value in the editor. A document with fields must be exported outside its source tree, and that check belongs to Pageformer.

## Preview

The editor previews the document with the styles resolved from the YAML template. It is visually close, not byte-identical to the PDF. The preview consumes `html::stylesheet`, the CSS generated from the template, instead of reparsing the YAML. The preview reuses the same class contract as the HTML backend: `body-paragraph`, `emphasis`, `strong`, `inline-code`, `strikethrough`, `quote`, `code-block`, `list-depth-<n>`, `task-done`, `task-todo`, `align-<side>` and the heading and table selectors. It does not need the PDF renderer.

## Round-trip risk

The editor serializes Markdown back to the source. The serialization must preserve the Pageformer syntax: `-pagebreak-`, `-!-`, `-horizontal-`, `{NAME}`, frontmatter, tables, lists, footnotes and alerts. A rich-text editor can normalize markers and escaping, so the fixtures under `assets/examples/` are the regression set. Keep the Markdown as the source of truth and test the round trip on every serializer change.

## Dependency

Pageweaver depends on Pageformer as a versioned library. When the work starts, at or after Pageformer 1.0.0, Pageformer is published on GitHub; Pageweaver uses a tag or a version, not a path between sibling folders. A local `[patch]` may point at the sibling checkout during development, but it is not the release architecture. Publishing to crates.io is a later option; the crate is `publish = false` today.

## Minimal GUI

Pageformer keeps a small egui window, `pageformer-publisher`, in the `publisher/` workspace member, for conversion and export only: drop or click the box to choose a Markdown file, pick a colored format from the column on the right, export and open the output, or use the `Copy` half of the TXT button to put the plain text on the clipboard. The window shows the resolved template, a status line and an Advanced section with a template override, the output directory, prefilled with the source folder and backed by a Choose folder button, and the confidential file; the selected file appears in a quiet baby blue, the window sizes itself to the content, and an embedded Liberation Sans Regular gives every label, button and tooltip the same regular-weight face. Every label, field and button has a tooltip on the name and the control. It has no editing, notebook, outline or live preview. Many users will not use the editor, and they still need a graphical way to export. The GUI is a thin consumer of the same library, not a second pipeline, and it runs the export on a worker thread.
