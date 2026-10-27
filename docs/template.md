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
version: 3

page:
  size: a4
  orientation: portrait
  margins:
    top: 85
    bottom: 57
    left: 85
    right: 57
  letterhead: images/letterhead.png

text:
  body:
    font_families:
      - Liberation Serif
      - DejaVu Serif
      - Libertinus Serif
    size: 12
  title:
    font_families:
      - Liberation Sans
      - DejaVu Sans
      - Libertinus Serif
    size: 18
  leading: 0.65
  paragraph_spacing: 24
  header_spacing: 18
  alignment: justified
```

Every section and every key is optional except `version`. Missing keys take the defaults described below, but an unknown key is an error, so a typo never changes the layout silently.

### `version`

The schema version, an integer. This build accepts `3` and rejects every other value. Version 3 replaced `text.font_families` and `text.size` with `text.body` and `text.title`, made a bare number mean points and added `text.header_spacing`. Version 2 had removed the `document` section: document metadata belongs to the Markdown.

### Document metadata

The template carries no title, author or language, so one template serves many documents. Those values come only from the Markdown YAML frontmatter:

```yaml
---
title: Contrato de Prestação de Serviços
author: Pageformer
language: pt-BR
---
```

`title` and `author` must be non-empty, and `language` must be a BCP 47 style tag such as `pt-BR`. Without a language, the PDF sets no language and Typst uses its default.

The frontmatter `title` is document metadata. The `title` style applies to level-1 headings, not to a generated cover title.

### `page`

- `size`: `a4`, `a5`, `letter`, `legal` or `oficio`, or a map with `width` and `height`. A custom size uses the same units as a length.
- `orientation`: `portrait` or `landscape`. Landscape swaps the width and the height.
- `margins`: `top`, `bottom`, `left` and `right`. A missing side defaults to `20mm` (`56.69pt`); a negative value is invalid.
- `letterhead`: a path relative to the template file. The file must be `png`, `jpg`, `gif`, `svg` or `webp`.

The default size is A4 in portrait orientation.

### `text`

- `body`: a map with `font_families` and `size` for body text. The default families are `Liberation Serif`, `DejaVu Serif` and `Libertinus Serif`; the default size is `12pt`.
- `title`: a map with `font_families` and `size` for level-1 headings. The default families are the same as the body, and the default size is `18pt`.
- `leading`: extra space between lines, in multiples of the font size. It must be finite and not negative; the default is `0.65`.
- `paragraph_spacing`: space between paragraphs. When omitted, Typst's default spacing applies (`1.2em`).
- `header_spacing`: space above a level-1 heading. When omitted, Typst's default heading spacing applies.
- `alignment`: `justified` or `left`. The default is `justified`.

Both `body` and `title` are optional maps. A present map may omit `font_families` or `size` independently, and the missing key takes its default.

## Lengths

A length is a number optionally followed by `mm`, `cm`, `in` or `pt`. A number without a unit is in points, so `30` and `30pt` are the same length. Negative values are rejected, and the font size must be greater than zero.

`leading` is the exception: it is a multiplier of the font size, not a length.

## Images

Images in the Markdown body resolve relative to the Markdown file, not to the template. Absolute paths are accepted. The supported formats are `png`, `jpg`, `gif`, `svg` and `webp`; `pdf` images are rejected because PDF/A output does not accept them. Remote images are rejected because the program never performs network requests.

## Current backend

The PDF backend uses the page fields, the body and title styles, leading, paragraph spacing, header spacing and alignment. The title style applies to level-1 headings, and `header_spacing` controls the space above them. Running headers and footers, per-level heading styles beyond level 1, heading page breaks and configurable table styling are not in the schema yet.

## Example

`assets/examples/.pageformer.yaml` is a commented template shared by both fixtures. It uses the fictional `Timbre.png` letterhead from the same directory and applies automatically because it is the nearest `.pageformer.yaml`:

```sh
pageformer assets/examples/Contrato.md assets/examples/Embargos.md
```

To use the same settings elsewhere, copy the file to the document directory as `.pageformer.yaml` or pass `--template`.

## Errors

The template is validated before any document is rendered. A YAML syntax error, an unknown key, an unsupported version, a rejected value or a negative spacing all stop the run with a diagnostic. The YAML parser reports the line and the column, and the loader adds the template path.
