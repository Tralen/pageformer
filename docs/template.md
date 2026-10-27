# Template

One YAML template drives every output format. The schema is versioned, and a template written for another version is rejected instead of being interpreted with different rules.

## Resolution

The nearest `.pageformer.yaml`, starting in the Markdown file's directory and walking upwards, wins entirely. There is no merge across directories. If the winning template is invalid, generation fails with a diagnostic that names the file and the position.

Without a local template, the global template in the user configuration directory is used:

- `$XDG_CONFIG_HOME/pageformer/pageformer.yaml`, when `XDG_CONFIG_HOME` is an absolute path;
- `$HOME/.config/pageformer/pageformer.yaml` otherwise;
- `%APPDATA%\pageformer\pageformer.yaml` on Windows.

Without either, the fallback embedded in the binary is used. It is the same file as `assets/default.pageformer.yaml`.

Paths written in the template, such as the letterhead image, resolve relative to the template file, never to the working directory.

## Schema

```yaml
version: "0.5"

page:
  size: a4
  orientation: portrait
  margins:
    top: 30mm
    bottom: 30mm
    left: 30mm
    right: 25mm
  letterhead: images/letterhead.png
  header:
    left: "{title}"
    right: "Page {page} of {pages}"
  footer:
    center: "{author}"

formatting:
  paragraph:
    font_families:
      - Linux Libertine O
      - Liberation Serif
    size: 12
    weight: normal
    style: normal
    leading: 7.8
    spacing: 24
    alignment: justified
    emphasis:
      style: italic
    strong:
      weight: bold
  heading:
    font_families:
      - Linux Biolinum O
      - Liberation Sans
    weight: bold
    style: normal
    spacing_before: 18
    spacing_after: 6
    h1:
      size: 18
    h2:
      size: 14
      spacing_before: 12
      spacing_after: 4
    h3:
      size: 12
      spacing_before: 10
      spacing_after: 4
    h4:
      size: 12
      spacing_before: 8
      spacing_after: 2
    h5:
      size: 12
      spacing_before: 8
      spacing_after: 2
    h6:
      size: 12
      spacing_before: 8
      spacing_after: 2

table:
  border: 0.5pt
  border_color: "#bbbbbb"
  header_fill: "#eeeeee"
  cell_padding: 6pt
```

Every section and every key is optional except `version`. Missing keys take the defaults described below, but an unknown key is an error, so a typo never changes the layout silently.

### `version`

The schema version is a string in the form `major.minor`, quoted so YAML cannot turn it into a float. This build accepts `"0.5"` and rejects every other value. Version 0.5 replaced the `text` section with `formatting`, made `paragraph` and `heading` explicit, added `weight` and `style`, and added the `emphasis` and `strong` inline overrides. The optional `page.header`, `page.footer` and `table` sections and the frontmatter `date`, `subject` and `keywords` were added inside version 0.5, because omitting them preserves the output of an existing template.

### Document metadata

The template carries no title, author or language, so one template serves many documents. Those values come only from the Markdown YAML frontmatter:

```yaml
---
title: Contrato de Prestação de Serviços
author: Pageformer
language: pt-BR
date: 2026-02-01
subject: Prestação de serviços
keywords:
  - contrato
  - prestação de serviços
---
```

`title`, `author`, `subject` and each keyword must be non-empty, and `language` must be a BCP 47 style tag such as `pt-BR`. Without a language, the PDF sets no language and Typst uses its default.

`date` is a calendar date written as `YYYY-MM-DD`, such as `2026-02-01`, and must be a real day. It replaces the fixed document date in the PDF metadata and drives the Typst world date, so the output stays deterministic. Without it, the fixed date `2026-01-01` is used.

`subject` reaches the PDF document description and `keywords` reaches its keyword list. Both are optional and omitted from the PDF when absent.

The frontmatter `title` is document metadata. The visible title is the first-level heading, styled by `formatting.heading.h1`.

### `page`

- `size`: `a4`, `a5`, `letter`, `legal` or `oficio`, or a map with `width` and `height`. A custom size uses the same units as a length.
- `orientation`: `portrait` or `landscape`. Landscape swaps the width and the height.
- `margins`: `top`, `bottom`, `left` and `right`. A missing side defaults to `20mm` (`56.69pt`); a negative value is invalid.
- `letterhead`: a path relative to the template file. The file must be `png`, `jpg`, `gif`, `svg` or `webp`.
- `header` and `footer`: running content above and below the body, each with `left`, `center` and `right` slot strings. The slots are the same on every page, while their tokens expand per page.

The default size is A4 in portrait orientation.

### Running header and footer

A `header` or `footer` map holds up to three slots. Only declared and non-blank slots are drawn; the other positions stay empty, so the remaining slots keep their place.

Slots expand four lowercase tokens from the document metadata:

| Token | Value |
|---|---|
| `{page}` | The current page number |
| `{pages}` | The page count of the whole document |
| `{title}` | The frontmatter title, empty when absent |
| `{author}` | The frontmatter author, empty when absent |

Any other lowercase token, such as `{nome}`, is a template error, because a typo must not reach the page as literal text. Uppercase tokens such as `{NAME}` stay literal here and are reserved for body secrets. Braces that do not form a lowercase token stay literal text.

The header and footer use the paragraph face. Their content sits inside the top and bottom margins, so a letterhead that already fills those margins needs no running content.

### `formatting.paragraph`

- `font_families`: a non-empty list of font names, tried in order. The default is `Liberation Serif`, `DejaVu Serif` and `Libertinus Serif`.
- `size`: a positive length. The default is `12pt`.
- `weight`: `thin`, `extralight`, `light`, `normal`, `medium`, `semibold`, `bold`, `extrabold` or `black`, or a number from 100 to 900. The default is `normal`.
- `style`: `normal` or `italic`. The default is `normal`.
- `leading`: extra space between lines, as a length. The default is `7.8pt`.
- `spacing`: space between paragraphs. When omitted, Typst's default spacing applies (`1.2em`).
- `alignment`: `justified` or `left`. The default is `justified`.
- `emphasis`: a map of overrides for `*...*` and `_..._`. It inherits from `paragraph`; its `style` defaults to `italic`.
- `strong`: a map of overrides for `**...**` and `__...__`. It inherits from `paragraph`; its `weight` defaults to `bold`.

Inside `emphasis` and `strong`, the allowed keys are `font_families`, `size`, `weight` and `style`. Only the declared keys override the paragraph values, so an empty map is valid and changes nothing beyond the semantic default.

### `formatting.heading`

The `heading` map is the base for every heading level:

- `font_families`: the heading fallback list. The default is `Liberation Sans`, `DejaVu Sans` and `Libertinus Serif`.
- `size`: a positive length. It applies to every level that does not override it.
- `weight`: the base weight. The default is `bold`.
- `style`: `normal` or `italic`. The default is `normal`.
- `spacing_before` and `spacing_after`: lengths around the heading. They apply to every level that does not override them.
- `page_break_before`: `true` starts the level on a new page. The break is weak: a heading already at the top of a page does not create a blank page. The default is `false`.

The `h1` to `h6` maps are optional overrides with the same keys. Only declared keys replace the base values. The default sizes are `18pt` for `h1`, `14pt` for `h2` and `12pt` for `h3` to `h6`; the default spacing is 18/6 for `h1`, 12/4 for `h2`, 10/4 for `h3` and 8/2 for `h4` to `h6`.

The template sets explicit values for each level. There is no hidden multiplier per level: what the YAML declares is what the PDF uses.

### `table`

The `table` section styles every table in the document:

- `border`: the border thickness as a length. The default is `0.5pt`; `0` removes the borders.
- `border_color`: a color written as `#rrggbb`. The default is `#bbbbbb`.
- `header_fill`: the background of the header row, as `#rrggbb`. When omitted, the header has no fill.
- `cell_padding`: the inset of every cell as a length. The default is `6pt`.

## Page breaks

A paragraph that contains only `{pagebreak}` starts a new page. The marker becomes a page break in the document model, so every paged backend reads the same block. Braces are ordinary text to CommonMark, so the Zed preview shows the marker and no raw HTML is involved. Any other occurrence, such as `{pagebreak}` inside a sentence, stays literal text. The backslash form `\pagebreak` is no longer special and stays literal text.

## Weights and styles

`weight` and `style` are independent, as in CSS and Typst. Weight is the stroke thickness; style is the slant. A font can offer regular, italic, bold and bold italic faces, and the PDF picks the closest face for the requested weight and style.

- Weight names map to 100 to 900; a bare number is also accepted in that range.
- `semibold` is 600, not a separate style.
- `style` accepts only `normal` and `italic`.

## Lengths

A length is a number optionally followed by `mm`, `cm`, `in` or `pt`. A number without a unit is in points, so `30` and `30pt` are the same length. Negative values are rejected, and a font size must be greater than zero.

## Images

Images in the Markdown body resolve relative to the Markdown file, not to the template. Absolute paths are accepted. The supported formats are `png`, `jpg`, `gif`, `svg` and `webp`; `pdf` images are rejected because PDF/A output does not accept them. Remote images are rejected because the program never performs network requests.

## Current backend

The PDF backend applies the paragraph face, leading, spacing and alignment. It emits one show rule per heading level with the resolved font, size, weight, style and spacing, and show rules for `emph` and `strong` from the inline overrides. A level with `page_break_before` also gets a weak page-break show rule. The backend draws the running header and footer, expands their tokens, applies the table style and explicit page breaks, and writes the frontmatter title, author, subject, keywords, language and date into the PDF metadata.

## Example

`assets/examples/.pageformer.yaml` is a commented template shared by the two legal fixtures. It uses the fictional `Timbre.png` letterhead from the same directory and applies automatically because it is the nearest `.pageformer.yaml`:

```sh
pageformer assets/examples/Contrato.md assets/examples/Embargos.md
```

`assets/examples/tecnico/.pageformer.yaml` is a second example, in a nested directory. It replaces the shared template for `Relatorio.md`, has no letterhead and demonstrates every running slot, the footer, the table style and a page break:

```sh
pageformer assets/examples/tecnico/Relatorio.md
```

To use the same settings elsewhere, copy the file to the document directory as `.pageformer.yaml` or pass `--template`.

## Errors

The template is validated before any document is rendered. A YAML syntax error, an unknown key, an unsupported version, a rejected weight, an unknown style, an invalid color, an unknown lowercase running token, a negative spacing or a missing font family all stop the run with a diagnostic. The YAML parser reports the line and the column, and the loader adds the template path.
