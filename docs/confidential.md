# Confidential

Confidential fields are `{NAME}` placeholders in the document body. Values never live in the Markdown, the template or the output path. They come from the environment or from an INI-style file selected for the document.

The template is deliberately unaware of the INI file. The selection lives in two user-level configuration levels, so a template can be copied or shared without exposing project names or secret paths.

## Two-level configuration

The public configuration in the user configuration directory holds the default output and lists the confidential directories:

- `$XDG_CONFIG_HOME/pageformer/config.yaml`, when `XDG_CONFIG_HOME` is absolute;
- `$HOME/.config/pageformer/config.yaml` otherwise;
- `%APPDATA%\pageformer\config.yaml` on Windows.

```yaml
version: 1

default_output: "/home/user/Documents/Pageformer"

confidential_directories:
  - "/home/user/Segredos/pageformer"
```

Both keys are optional. A file with only `default_output` is valid, and a missing file means no default output and no confidential mapping.

Each confidential directory contains the sensitive map and the INI files:

```text
/home/user/Segredos/pageformer/
├── confidential.yaml
├── escritorio.ini
└── tecnico.ini
```

```yaml
version: 1

files:
  escritorio: "/home/user/Documentos/escritorio"
  tecnico: "/home/user/Documentos/tecnico"
  "escritório particular.ini": "/home/user/Documentos/particular"

outputs:
  escritorio: "/home/user/Documentos/publicacao/escritorio"
  tecnico: "/home/user/Documentos/publicacao/tecnico"
```

The key is the INI file name. When it has no `.ini` suffix, the suffix is implicit: `escritorio` means `escritorio.ini` in the same directory. When the key already ends with `.ini`, that exact file name is used, so an explicit name is possible. The value is the absolute source folder served by that INI.

The file name may contain spaces, including `escritório particular.ini`; quote the key in YAML when it has spaces or punctuation. Leading or trailing spaces are rejected, as are path separators, `:`, `*`, `?`, `"`, `<`, `>`, `|`, control characters and names that are only dots. An explicit `.INI` suffix is accepted and the case is preserved.

The inner map is sensitive because it reveals which folders belong to which client, case or project. That is why it stays inside the confidential directory instead of the general configuration.

## Permissions

- The confidential directory must be `700`.
- `confidential.yaml` and every INI must be `600`.
- The public `config.yaml` should also be `600`, although only paths appear there.

On Unix these modes are checked. Windows relies on the ACL of the user profile.

The INI file must still stay outside the source tree of the document, as with `--confidential-file`; the loader checks that when the document is processed.

## Resolution

The source root of the job is canonicalized and compared with each mapped folder. The longest folder prefix wins, so a mapping for `/home/user/Documentos` covers its subfolders, while a more specific mapping for `/home/user/Documentos/escritorio` takes precedence.

Two files cannot map the same source folder, and two entries cannot list the same confidential directory. An ambiguous map is an error instead of a silent choice.

The INI name is a single file name with an optional `.ini` suffix; it cannot contain a path separator. Paths in both configuration files must be absolute. A `confidential.yaml` or INI that escapes its confidential directory through a symlink is rejected.

## Output directories

The output is a directory, not a file name. The format extension still comes from `--format`, and the existing rules apply: a directory input mirrors its relative tree, and a direct file is written flat. The program creates a missing output directory.

The command resolves the output per input, in this order:

1. `--output` on the command line.
2. The `outputs` entry for the INI selected by the source root.
3. `default_output` in the public `config.yaml`.
4. Beside the Markdown source, the current behavior.

A more specific source mapping wins entirely: if its entry has no `outputs` value, the resolver falls back to `default_output`, not to the output of a broader mapping. The mapping uses the input root (the directory argument, or the parent of a direct file), not the parent of each file.

The `outputs` value for an INI must be outside the mapped source folder. The loader rejects the configuration otherwise, so a confidential project cannot accidentally write beside its own source. `default_output` may point anywhere; for a document with confidential fields, the existing export check still rejects an output inside the source tree.

The `.ini` suffix in an `outputs` key is optional and normalized like in `files`. Every `outputs` key must have a matching `files` entry; an unknown key, a duplicate after normalization, a relative path or a path with a separator is an error. `--check` ignores all output settings.

If `--confidential-file` is used, the folder mapping and the per-INI outputs are ignored, but the public `config.yaml` is still read for `default_output`. A malformed public configuration therefore stops the run even with an explicit file.

## Precedence

1. `--confidential-file` on the command line.
2. The confidential field in `pageformer-publisher`.
3. The folder mapping in the confidential directories.
4. No file, with only the `PAGEFORMER_CONFIDENTIAL_*` variables.

The environment is read first, and an INI file overrides it for the names it declares. An explicit file wins over the folder mapping and is loaded once for the run.

## Failures

A missing public configuration is the normal case and means no mapping. An existing but invalid public configuration, a missing confidential directory, a missing `confidential.yaml`, a missing mapped INI, invalid permissions, an ambiguous folder, or an unknown, duplicate or relative output all stop the run with a diagnostic. An output for an INI must be outside its source folder; one inside it stops the run as well.

For a batch, an INI mapping that fails to load affects the documents that use it; the other documents continue. The run exits with failure when any document fails.

## Values

The INI format is one `NAME = value` per line, with `#` or `;` starting a comment. A field must stay within one text run: inline formatting or a line break inside the name splits it, and the parts stay literal text. An unknown field is an error, never text left in the final document. `--check` lists the field names only. Values never appear in the terminal output, the warnings, the PDF XMP metadata, the office properties, the HTML head, the EPUB OPF, the navigation or an error message.

## GUI and Pageweaver

`pageformer-publisher` resolves the mapping when its Advanced confidential field is empty. The explicit field remains the override. The same resolution is used by the TXT copy action. The window still prefills its output field with the source folder; using `default_output` or the per-INI output in that field is a later step.

Pageweaver exports through the same pipeline, so it should call the same resolver. The preview must never display a confidential value; the resolution belongs to the export step.
