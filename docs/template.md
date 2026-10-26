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
version: 2

page:
  size: a4
  orientation: portrait
  margins:
    top: 25mm
    bottom: 20mm
    left: 25mm
    right: 20mm
  letterhead: images/letterhead.png

text:
  font_families:
    - Liberation Serif
    - DejaVu Serif
    - Libertinus Serif
  size: 12pt
  leading: 0.65
  alignment: justified
```

Every section and every key is optional except `version`. Missing keys take the defaults described below, but an unknown key is an error, so a typo never changes the layout silently.

### `version`

The schema version, an integer. This build accepts `2` and rejects every other value. Version 2 removed the `document` section: document metadata belongs to the Markdown.

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

### `page`

- `size`: `a4`, `a5`, `letter`, `legal` or `oficio`, or a map with `width` and `height`. A custom size uses the same units as a length.
- `orientation`: `portrait` or `landscape`. Landscape swaps the width and the height.
- `margins`: `top`, `bottom`, `left` and `right`. A missing side defaults to `20mm`; a negative value is invalid.
- `letterhead`: a path relative to the template file. The file must be `png`, `jpg`, `gif`, `svg` or `webp`.

The default size is A4 in portrait orientation.

### `text`

- `font_families`: a non-empty list of font names, tried in order. The default is `Liberation Serif`, `DejaVu Serif` and `Libertinus Serif`; the last one is embedded in the binary and is always available.
- `size`: a positive length. The default is `12pt`.
- `leading`: extra space between lines, in multiples of the font size. It must be finite and not negative; the default is `0.65`.
- `alignment`: `justified` or `left`. The default is `justified`.

## Lengths

A length is a number followed by one of `mm`, `cm`, `in` or `pt`. The unit is mandatory: `30` is an error and `30mm` is valid. Negative values are rejected, and the font size must be greater than zero.

## Images

Images in the Markdown body resolve relative to the Markdown file, not to the template. Absolute paths are accepted. The supported formats are `png`, `jpg`, `gif`, `svg` and `webp`; `pdf` images are rejected because PDF/A output does not accept them. Remote images are rejected because the program never performs network requests.

## Current backend

The PDF backend uses the page and text fields above and reads the document metadata from the frontmatter. Header and footer content, heading page breaks, heading styles and configurable table styling are not in the schema yet, so the generated PDF uses Typst's default headings and lists, a neutral table grid and no header or footer.

## Example

`assets/examples/Embargos.pageformer.yaml` is a commented template for the legal fixture. Copy it to `.pageformer.yaml` next to the Markdown to have it discovered, or pass it explicitly:

```sh
pageformer --template assets/examples/Embargos.pageformer.yaml assets/examples/Embargos.md
```

## Errors

The template is validated before any document is rendered. A YAML syntax error, an unknown key, an unsupported version, a missing unit or a rejected value all stop the run with a diagnostic. The YAML parser reports the line and the column, and the loader adds the template path.
