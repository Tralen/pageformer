//! Loads confidential values from the environment and an INI-style file.
//!
//! The file path is the only confidential argument accepted on the command
//! line. A value is never read from an argument, never printed, and never
//! included in an error.
//!
//! The format is one `NAME = value` per line. Whitespace around the name, the
//! `=` and the value is trimmed; a value keeps every `=` after the first one and
//! any `#` or `;` that does not start the line. Lines whose first non-blank
//! character is `#` or `;` are comments. Sections, quotes, escapes and
//! interpolation are not part of the format.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{ConfidentialName, ConfidentialStore, ConfidentialValue, normalize_absolute};

/// The prefix of the environment variables read for confidential values.
pub const ENV_PREFIX: &str = "PAGEFORMER_CONFIDENTIAL_";

/// Why a confidential store could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    NotAFile {
        path: PathBuf,
    },
    Permissions {
        path: PathBuf,
    },
    InsideSource {
        path: PathBuf,
        root: PathBuf,
    },
    InvalidLine {
        path: PathBuf,
        line: usize,
    },
    InvalidName {
        path: PathBuf,
        line: usize,
    },
    Duplicate {
        path: PathBuf,
        line: usize,
        name: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "cannot read confidential file {}: {source}",
                    path.display()
                )
            }
            Self::NotAFile { path } => write!(
                formatter,
                "confidential file {} is not a regular file",
                path.display()
            ),
            Self::Permissions { path } => write!(
                formatter,
                "confidential file {} must not be readable by group or others; use chmod 600",
                path.display()
            ),
            Self::InsideSource { path, root } => write!(
                formatter,
                "confidential file {} is inside the source tree {}",
                path.display(),
                root.display()
            ),
            Self::InvalidLine { path, line } => write!(
                formatter,
                "confidential file {} line {line}: a line must be `NAME = value`",
                path.display()
            ),
            Self::InvalidName { path, line } => write!(
                formatter,
                "confidential file {} line {line}: a field name must use uppercase letters, digits or underscores",
                path.display()
            ),
            Self::Duplicate { path, line, name } => write!(
                formatter,
                "confidential file {} line {line}: the field `{name}` is defined twice",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::NotAFile { .. }
            | Self::Permissions { .. }
            | Self::InsideSource { .. }
            | Self::InvalidLine { .. }
            | Self::InvalidName { .. }
            | Self::Duplicate { .. } => None,
        }
    }
}

/// Reads the environment and, when given, a confidential file.
///
/// The environment is read first and the file overrides it. Every source root
/// is canonicalized; the file must resolve outside all of them.
pub fn load(path: Option<&Path>, source_roots: &[&Path]) -> Result<ConfidentialStore, LoadError> {
    build(path, source_roots, environment())
}

fn environment() -> Vec<(ConfidentialName, ConfidentialValue)> {
    std::env::vars_os()
        .filter_map(|(key, value)| {
            let key = key.to_str()?;
            let name = key.strip_prefix(ENV_PREFIX)?;
            let name = ConfidentialName::parse(name)?;
            let value = value.to_str()?;
            Some((name, ConfidentialValue::new(value)))
        })
        .collect()
}

fn build(
    path: Option<&Path>,
    source_roots: &[&Path],
    environment: Vec<(ConfidentialName, ConfidentialValue)>,
) -> Result<ConfidentialStore, LoadError> {
    let mut store = ConfidentialStore::new();
    for (name, value) in environment {
        store.insert(name, value);
    }
    if let Some(path) = path {
        load_file(path, source_roots, &mut store)?;
    }
    Ok(store)
}

fn load_file(
    path: &Path,
    source_roots: &[&Path],
    store: &mut ConfidentialStore,
) -> Result<(), LoadError> {
    let metadata = fs::metadata(path).map_err(|source| LoadError::Read {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(LoadError::NotAFile {
            path: path.to_owned(),
        });
    }
    check_permissions(path, &metadata)?;
    let canonical = path.canonicalize().map_err(|source| LoadError::Read {
        path: path.to_owned(),
        source,
    })?;
    for root in source_roots {
        let normalized = normalize_absolute(root).ok_or_else(|| LoadError::InsideSource {
            path: path.to_owned(),
            root: (*root).to_owned(),
        })?;
        if canonical.starts_with(&normalized) {
            return Err(LoadError::InsideSource {
                path: path.to_owned(),
                root: normalized,
            });
        }
    }
    let text = fs::read_to_string(&canonical).map_err(|source| LoadError::Read {
        path: path.to_owned(),
        source,
    })?;
    parse_file(&text, path, store)
}

fn parse_file(text: &str, path: &Path, store: &mut ConfidentialStore) -> Result<(), LoadError> {
    let mut seen = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((name, value)) = raw.split_once('=') else {
            return Err(LoadError::InvalidLine {
                path: path.to_owned(),
                line,
            });
        };
        let Some(name) = ConfidentialName::parse(name.trim()) else {
            return Err(LoadError::InvalidName {
                path: path.to_owned(),
                line,
            });
        };
        if seen.insert(name.clone(), ()).is_some() {
            return Err(LoadError::Duplicate {
                path: path.to_owned(),
                line,
                name: name.as_str().to_owned(),
            });
        }
        store.insert(name, ConfidentialValue::new(value.trim()));
    }
    Ok(())
}

#[cfg(unix)]
fn check_permissions(path: &Path, metadata: &fs::Metadata) -> Result<(), LoadError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 != 0 {
        Err(LoadError::Permissions {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn check_permissions(_path: &Path, _metadata: &fs::Metadata) -> Result<(), LoadError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn name(text: &str) -> ConfidentialName {
        ConfidentialName::parse(text).expect("a valid name")
    }

    fn value(text: &str) -> ConfidentialValue {
        ConfidentialValue::new(text)
    }

    fn write_file(path: &Path, contents: &str) {
        fs::write(path, contents).expect("the file must be written");
        make_private(path);
    }

    #[cfg(unix)]
    fn make_private(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("the permissions must change");
    }

    #[cfg(not(unix))]
    fn make_private(_path: &Path) {}

    #[test]
    fn the_environment_is_read_with_the_prefix() {
        let store = build(
            None,
            &[],
            vec![(name("NOME"), value("Fulano")), (name("CPF"), value("000"))],
        )
        .expect("the store");
        assert_eq!(store.names().count(), 2);
        assert_eq!(
            store.get(&name("NOME")).map(ConfidentialValue::expose),
            Some("Fulano")
        );
    }

    #[test]
    fn the_file_overrides_the_environment() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(&file, "NOME=Arquivo\n");
        let store =
            build(Some(&file), &[], vec![(name("NOME"), value("Ambiente"))]).expect("the store");
        assert_eq!(
            store.get(&name("NOME")).map(ConfidentialValue::expose),
            Some("Arquivo")
        );
    }

    #[test]
    fn comments_blank_lines_crlf_and_empty_values_are_accepted() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(
            &file,
            "# comment\r\n; another comment\r\n\r\nNOME=Fulano de Tal\r\nVAZIO=\r\nSENHA=a=b\r\n",
        );
        let store = build(Some(&file), &[], Vec::new()).expect("the store");
        assert_eq!(store.names().count(), 3);
        assert_eq!(
            store.get(&name("SENHA")).map(ConfidentialValue::expose),
            Some("a=b")
        );
        assert_eq!(
            store.get(&name("VAZIO")).map(ConfidentialValue::expose),
            Some("")
        );
    }

    #[test]
    fn whitespace_around_the_separator_is_trimmed() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(
            &file,
            "  NOME   =   Fulano de Tal  \n\tCPF\t=\t000.000.000-00\n",
        );
        let store = build(Some(&file), &[], Vec::new()).expect("the store");
        assert_eq!(
            store.get(&name("NOME")).map(ConfidentialValue::expose),
            Some("Fulano de Tal")
        );
        assert_eq!(
            store.get(&name("CPF")).map(ConfidentialValue::expose),
            Some("000.000.000-00")
        );
    }

    #[test]
    fn a_value_keeps_separators_and_comment_characters_after_the_first() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(
            &file,
            "OAB = OAB/DF nº 88.666\nEQUACAO = a=b # literal\nFIM = valor; com ponto e vírgula\n",
        );
        let store = build(Some(&file), &[], Vec::new()).expect("the store");
        assert_eq!(
            store.get(&name("OAB")).map(ConfidentialValue::expose),
            Some("OAB/DF nº 88.666")
        );
        assert_eq!(
            store.get(&name("EQUACAO")).map(ConfidentialValue::expose),
            Some("a=b # literal")
        );
        assert_eq!(
            store.get(&name("FIM")).map(ConfidentialValue::expose),
            Some("valor; com ponto e vírgula")
        );
    }

    #[test]
    fn a_section_header_is_rejected() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(&file, "[confidencial]\nNOME = Fulano\n");
        let error = build(Some(&file), &[], Vec::new()).expect_err("a section header");
        assert!(matches!(error, LoadError::InvalidLine { line: 1, .. }));
        assert!(!error.to_string().contains("Fulano"), "{error}");
    }

    #[test]
    fn an_invalid_line_is_reported_by_number() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(&file, "NOME=Fulano\nsem sinal\n");
        let error = build(Some(&file), &[], Vec::new()).expect_err("an invalid line");
        assert!(matches!(error, LoadError::InvalidLine { line: 2, .. }));
        assert!(error.to_string().contains("line 2"), "{error}");
        assert!(!error.to_string().contains("Fulano"), "{error}");
    }

    #[test]
    fn an_invalid_name_is_reported_without_the_value() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(&file, "nome=Fulano\n");
        let error = build(Some(&file), &[], Vec::new()).expect_err("an invalid name");
        assert!(matches!(error, LoadError::InvalidName { line: 1, .. }));
        assert!(!error.to_string().contains("Fulano"), "{error}");
    }

    #[test]
    fn a_duplicate_field_is_rejected() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(&file, "NOME=A\nNOME=B\n");
        let error = build(Some(&file), &[], Vec::new()).expect_err("a duplicate");
        assert!(matches!(error, LoadError::Duplicate { line: 2, .. }));
        assert!(error.to_string().contains("NOME"), "{error}");
    }

    #[test]
    fn a_file_inside_a_source_tree_is_rejected() {
        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        write_file(&file, "NOME=A\n");
        let error = build(Some(&file), &[project.path()], Vec::new())
            .expect_err("a file inside the source tree");
        assert!(matches!(error, LoadError::InsideSource { .. }));
    }

    #[test]
    fn a_file_outside_the_source_tree_is_accepted() {
        let source = TempDir::new().expect("a source");
        let values = TempDir::new().expect("a values directory");
        let file = values.path().join("values.ini");
        write_file(&file, "NOME=A\n");
        let store = build(Some(&file), &[source.path()], Vec::new()).expect("the store");
        assert_eq!(
            store.get(&name("NOME")).map(ConfidentialValue::expose),
            Some("A")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_readable_file_is_rejected() {
        use std::os::unix::fs::PermissionsExt;

        let project = TempDir::new().expect("a project");
        let file = project.path().join("values.ini");
        fs::write(&file, "NOME=A\n").expect("the file must be written");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644))
            .expect("the permissions must change");
        let error = build(Some(&file), &[], Vec::new()).expect_err("a readable file");
        assert!(matches!(error, LoadError::Permissions { .. }));
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let project = TempDir::new().expect("a project");
        let error = build(Some(&project.path().join("missing.ini")), &[], Vec::new())
            .expect_err("a missing file");
        assert!(matches!(error, LoadError::Read { .. }));
    }
}
