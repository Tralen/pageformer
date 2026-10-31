//! The user-level map from source folders to confidential INI files.
//!
//! The public `config.yaml` only lists confidential directories. Each directory
//! owns a `confidential.yaml` that names its INI files and the source folders
//! they serve. Keeping the mapping inside the confidential directory keeps the
//! project names out of the general configuration.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::normalize_absolute;

/// The public configuration file in the user configuration directory.
pub const PUBLIC_FILE_NAME: &str = "config.yaml";

/// The file inside each confidential directory.
pub const SECRET_FILE_NAME: &str = "confidential.yaml";

/// The version accepted by both configuration files.
pub const CONFIG_VERSION: u32 = 1;

/// One source folder and the INI file that serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    source: PathBuf,
    ini: PathBuf,
}

/// The resolved folder-to-INI map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfidentialConfig {
    entries: Vec<Entry>,
}

impl ConfidentialConfig {
    /// Loads the public configuration from the user configuration directory.
    ///
    /// A missing file is an empty configuration, not an error.
    pub fn load_default() -> Result<Self, ConfigError> {
        let Some(directory) = crate::config::user_config_directory() else {
            return Ok(Self::default());
        };
        let path = directory.join(PUBLIC_FILE_NAME);
        match fs::metadata(&path) {
            Ok(_) => Self::from_public_file(&path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::PublicRead { path, source }),
        }
    }

    /// Loads a public configuration from an explicit path.
    pub fn from_public_file(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::PublicRead {
            path: path.to_owned(),
            source,
        })?;
        let file: PublicFile =
            serde_yaml::from_str(&text).map_err(|source| ConfigError::PublicParse {
                path: path.to_owned(),
                source,
            })?;
        if file.version != CONFIG_VERSION {
            return Err(ConfigError::Version {
                path: path.to_owned(),
                found: file.version,
            });
        }

        let mut directories = BTreeSet::new();
        let mut entries = Vec::new();
        for directory in file.confidential_directories {
            let path = PathBuf::from(&directory);
            if !path.is_absolute() {
                return Err(ConfigError::NotAbsolute { path });
            }
            let normalized = normalize_absolute(&path)
                .ok_or_else(|| ConfigError::NotAbsolute { path: path.clone() })?;
            let metadata =
                fs::metadata(&normalized).map_err(|source| ConfigError::DirectoryRead {
                    path: normalized.clone(),
                    source,
                })?;
            if !metadata.is_dir() {
                return Err(ConfigError::NotDirectory { path: normalized });
            }
            check_directory_permissions(&normalized, &metadata)?;
            if !directories.insert(normalized.clone()) {
                return Err(ConfigError::DuplicateDirectory { path: normalized });
            }
            entries.extend(load_secret_directory(&normalized)?);
        }

        let mut sources = BTreeMap::new();
        for entry in &entries {
            if sources
                .insert(entry.source.clone(), entry.ini.clone())
                .is_some()
            {
                return Err(ConfigError::DuplicateFolder {
                    folder: entry.source.clone(),
                });
            }
        }
        entries.sort_by(|left, right| {
            right
                .source
                .components()
                .count()
                .cmp(&left.source.components().count())
                .then_with(|| right.source.cmp(&left.source))
        });
        Ok(Self { entries })
    }

    /// The INI file for a source folder, using the longest matching prefix.
    pub fn file_for(&self, source_root: &Path) -> Option<&Path> {
        let root = normalize_absolute(source_root)?;
        self.entries
            .iter()
            .find(|entry| root.starts_with(&entry.source))
            .map(|entry| entry.ini.as_path())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicFile {
    version: u32,
    #[serde(default)]
    confidential_directories: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretFile {
    version: u32,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IniName(String);

impl IniName {
    fn parse(text: &str) -> Option<Self> {
        if text.is_empty()
            || text.trim() != text
            || text == "."
            || text == ".."
            || text.ends_with('.')
            || text.chars().all(|character| character == '.')
            || text.chars().any(|character| {
                character.is_control()
                    || matches!(
                        character,
                        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                    )
            })
        {
            return None;
        }
        let name = if text.to_ascii_lowercase().ends_with(".ini") {
            text.to_owned()
        } else {
            format!("{text}.ini")
        };
        let stem = name.get(..name.len().saturating_sub(4))?;
        if stem.is_empty()
            || stem.trim().is_empty()
            || stem.chars().all(|character| character == '.')
        {
            return None;
        }
        Some(Self(name))
    }

    fn file_name(&self) -> &str {
        &self.0
    }
}

fn load_secret_directory(directory: &Path) -> Result<Vec<Entry>, ConfigError> {
    let config_path = directory.join(SECRET_FILE_NAME);
    let canonical = fs::canonicalize(&config_path).map_err(|source| ConfigError::SecretRead {
        path: config_path.clone(),
        source,
    })?;
    if !canonical.starts_with(directory) {
        return Err(ConfigError::OutsideSecret {
            path: canonical,
            directory: directory.to_owned(),
        });
    }
    let metadata = fs::metadata(&canonical).map_err(|source| ConfigError::SecretRead {
        path: canonical.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(ConfigError::NotFile { path: canonical });
    }
    check_file_permissions(&canonical, &metadata)?;

    let text = fs::read_to_string(&canonical).map_err(|source| ConfigError::SecretRead {
        path: canonical.clone(),
        source,
    })?;
    let file: SecretFile =
        serde_yaml::from_str(&text).map_err(|source| ConfigError::SecretParse {
            path: canonical.clone(),
            source,
        })?;
    if file.version != CONFIG_VERSION {
        return Err(ConfigError::Version {
            path: canonical,
            found: file.version,
        });
    }

    let mut entries = Vec::with_capacity(file.files.len());
    for (name, folder) in file.files {
        let name = IniName::parse(&name)
            .ok_or_else(|| ConfigError::InvalidIniName { name: name.clone() })?;
        let folder = PathBuf::from(folder);
        if !folder.is_absolute() {
            return Err(ConfigError::NotAbsolute { path: folder });
        }
        let source = normalize_absolute(&folder).ok_or_else(|| ConfigError::NotAbsolute {
            path: folder.clone(),
        })?;
        if let Ok(metadata) = fs::metadata(&source)
            && !metadata.is_dir()
        {
            return Err(ConfigError::NotDirectory { path: source });
        }
        let ini_path = directory.join(name.file_name());
        let ini = fs::canonicalize(&ini_path).map_err(|source_error| ConfigError::IniRead {
            path: ini_path.clone(),
            source: source_error,
        })?;
        if !ini.starts_with(directory) {
            return Err(ConfigError::OutsideSecret {
                path: ini,
                directory: directory.to_owned(),
            });
        }
        let metadata = fs::metadata(&ini).map_err(|source_error| ConfigError::IniRead {
            path: ini.clone(),
            source: source_error,
        })?;
        if !metadata.is_file() {
            return Err(ConfigError::NotFile { path: ini });
        }
        check_file_permissions(&ini, &metadata)?;
        entries.push(Entry { source, ini });
    }
    Ok(entries)
}

#[cfg(unix)]
fn check_file_permissions(path: &Path, metadata: &fs::Metadata) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 != 0 {
        Err(ConfigError::FilePermissions {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn check_file_permissions(_path: &Path, _metadata: &fs::Metadata) -> Result<(), ConfigError> {
    Ok(())
}

#[cfg(unix)]
fn check_directory_permissions(path: &Path, metadata: &fs::Metadata) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 != 0 {
        Err(ConfigError::DirectoryPermissions {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

#[cfg(not(unix))]
fn check_directory_permissions(_path: &Path, _metadata: &fs::Metadata) -> Result<(), ConfigError> {
    Ok(())
}

/// Why the confidential configuration could not be loaded.
#[derive(Debug)]
pub enum ConfigError {
    PublicRead {
        path: PathBuf,
        source: io::Error,
    },
    PublicParse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    SecretRead {
        path: PathBuf,
        source: io::Error,
    },
    SecretParse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    IniRead {
        path: PathBuf,
        source: io::Error,
    },
    DirectoryRead {
        path: PathBuf,
        source: io::Error,
    },
    Version {
        path: PathBuf,
        found: u32,
    },
    NotAbsolute {
        path: PathBuf,
    },
    NotDirectory {
        path: PathBuf,
    },
    NotFile {
        path: PathBuf,
    },
    OutsideSecret {
        path: PathBuf,
        directory: PathBuf,
    },
    FilePermissions {
        path: PathBuf,
    },
    DirectoryPermissions {
        path: PathBuf,
    },
    InvalidIniName {
        name: String,
    },
    DuplicateDirectory {
        path: PathBuf,
    },
    DuplicateFolder {
        folder: PathBuf,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicRead { path, source } => write!(
                formatter,
                "cannot read confidential configuration {}: {source}",
                path.display()
            ),
            Self::PublicParse { path, source } => write!(
                formatter,
                "confidential configuration {}: {source}",
                path.display()
            ),
            Self::SecretRead { path, source } => write!(
                formatter,
                "cannot read confidential directory map {}: {source}",
                path.display()
            ),
            Self::SecretParse { path, source } => write!(
                formatter,
                "confidential directory map {}: {source}",
                path.display()
            ),
            Self::IniRead { path, source } => write!(
                formatter,
                "cannot read confidential file {}: {source}",
                path.display()
            ),
            Self::DirectoryRead { path, source } => write!(
                formatter,
                "cannot read confidential directory {}: {source}",
                path.display()
            ),
            Self::Version { path, found } => write!(
                formatter,
                "confidential configuration {} uses version {found}; expected {CONFIG_VERSION}",
                path.display()
            ),
            Self::NotAbsolute { path } => write!(
                formatter,
                "the confidential configuration path {} must be absolute",
                path.display()
            ),
            Self::NotDirectory { path } => write!(
                formatter,
                "the confidential configuration path {} must be a directory",
                path.display()
            ),
            Self::NotFile { path } => write!(
                formatter,
                "the confidential configuration path {} must be a regular file",
                path.display()
            ),
            Self::OutsideSecret { path, directory } => write!(
                formatter,
                "the confidential configuration path {} escapes its directory {}",
                path.display(),
                directory.display()
            ),
            Self::FilePermissions { path } => write!(
                formatter,
                "confidential file {} must not be readable by group or others; use chmod 600",
                path.display()
            ),
            Self::DirectoryPermissions { path } => write!(
                formatter,
                "confidential directory {} must not be accessible by group or others; use chmod 700",
                path.display()
            ),
            Self::InvalidIniName { name } => write!(
                formatter,
                "confidential file name `{name}` must be a single file name without path separators or reserved characters; the .ini suffix is optional"
            ),
            Self::DuplicateDirectory { path } => write!(
                formatter,
                "the confidential directory {} is listed more than once",
                path.display()
            ),
            Self::DuplicateFolder { folder } => write!(
                formatter,
                "the source folder {} is mapped by more than one confidential file",
                folder.display()
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PublicRead { source, .. }
            | Self::SecretRead { source, .. }
            | Self::IniRead { source, .. }
            | Self::DirectoryRead { source, .. } => Some(source),
            Self::PublicParse { source, .. } | Self::SecretParse { source, .. } => Some(source),
            Self::Version { .. }
            | Self::NotAbsolute { .. }
            | Self::NotDirectory { .. }
            | Self::NotFile { .. }
            | Self::OutsideSecret { .. }
            | Self::FilePermissions { .. }
            | Self::DirectoryPermissions { .. }
            | Self::InvalidIniName { .. }
            | Self::DuplicateDirectory { .. }
            | Self::DuplicateFolder { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn secret_directory() -> TempDir {
        let directory = TempDir::new().expect("a confidential directory");
        set_mode(directory.path(), 0o700);
        directory
    }

    fn yaml_string(text: &str) -> String {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }

    fn write_public(path: &Path, directories: &[&Path]) {
        let mut text = String::from("version: 1\nconfidential_directories:\n");
        for directory in directories {
            text.push_str(&format!(
                "  - {}\n",
                yaml_string(&directory.display().to_string())
            ));
        }
        fs::write(path, text).expect("the public configuration must be written");
    }

    fn write_secret(directory: &Path, files: &[(&str, &Path)]) {
        let mut text = String::from("version: 1\nfiles:\n");
        for (name, folder) in files {
            text.push_str(&format!(
                "  {}: {}\n",
                yaml_string(name),
                yaml_string(&folder.display().to_string())
            ));
        }
        let path = directory.join(SECRET_FILE_NAME);
        fs::write(&path, text).expect("the secret map must be written");
        set_mode(&path, 0o600);
    }

    fn write_ini(directory: &Path, name: &str) {
        write_ini_file(directory, &format!("{name}.ini"));
    }

    fn write_ini_file(directory: &Path, file_name: &str) {
        let path = directory.join(file_name);
        fs::write(&path, "NOME = Fulano\n").expect("the INI must be written");
        set_mode(&path, 0o600);
    }

    fn set_mode(path: &Path, mode: u32) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .expect("the permissions must change");
        }
        #[cfg(not(unix))]
        {
            let _ = (path, mode);
        }
    }

    fn project() -> TempDir {
        TempDir::new().expect("a source directory")
    }

    /// The configuration canonicalizes its paths, so the expected paths must be
    /// canonical too; on macOS `/var` resolves to `/private/var`.
    fn resolved(path: &Path) -> PathBuf {
        fs::canonicalize(path).expect("the path must resolve")
    }

    fn config(temp: &TempDir, secret: &Path, source: &Path) -> ConfidentialConfig {
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret]);
        write_secret(secret, &[("escritorio", source)]);
        write_ini(secret, "escritorio");
        ConfidentialConfig::from_public_file(&public).expect("the configuration must load")
    }

    #[test]
    fn a_mapped_folder_uses_its_ini() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let config = config(&temp, secret.path(), source.path());

        let expected = resolved(&secret.path().join("escritorio.ini"));
        assert_eq!(config.file_for(source.path()), Some(expected.as_path()));
        assert_eq!(
            config.file_for(&source.path().join("nested")),
            Some(expected.as_path())
        );
    }

    #[test]
    fn the_longest_folder_prefix_wins() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let nested = source.path().join("nested");
        fs::create_dir(&nested).expect("the nested folder must exist");
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret(
            secret.path(),
            &[("outside", source.path()), ("inside", &nested)],
        );
        write_ini(secret.path(), "outside");
        write_ini(secret.path(), "inside");
        let config =
            ConfidentialConfig::from_public_file(&public).expect("the configuration must load");

        let inside = resolved(&secret.path().join("inside.ini"));
        let outside = resolved(&secret.path().join("outside.ini"));
        assert_eq!(config.file_for(&nested), Some(inside.as_path()));
        assert_eq!(config.file_for(source.path()), Some(outside.as_path()));
    }

    #[test]
    fn the_ini_suffix_is_implicit_but_can_be_written_explicitly() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let nested = source.path().join("nested");
        fs::create_dir(&nested).expect("the nested folder must exist");
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret(
            secret.path(),
            &[
                ("escritorio", source.path()),
                ("escritório particular.ini", &nested),
            ],
        );
        write_ini(secret.path(), "escritorio");
        write_ini_file(secret.path(), "escritório particular.ini");
        let config =
            ConfidentialConfig::from_public_file(&public).expect("the configuration must load");

        let particular = resolved(&secret.path().join("escritório particular.ini"));
        let escritorio = resolved(&secret.path().join("escritorio.ini"));
        assert_eq!(config.file_for(&nested), Some(particular.as_path()));
        assert_eq!(config.file_for(source.path()), Some(escritorio.as_path()));
    }

    #[test]
    fn ini_names_keep_inner_spaces_and_the_optional_suffix() {
        assert_eq!(
            IniName::parse("escritório particular")
                .expect("an implicit name")
                .file_name(),
            "escritório particular.ini"
        );
        assert_eq!(
            IniName::parse("escritório particular.ini")
                .expect("an explicit name")
                .file_name(),
            "escritório particular.ini"
        );
        assert_eq!(
            IniName::parse("escritório particular.INI")
                .expect("an explicit upper-case suffix")
                .file_name(),
            "escritório particular.INI"
        );
        for invalid in [
            ".", "..", ".ini", "foo.", "foo ", " foo", "foo/bar", "foo\\bar", "foo:bar", "foo*bar",
            "foo\nbar",
        ] {
            assert!(IniName::parse(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn an_unmapped_folder_has_no_file() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let config = config(&temp, secret.path(), source.path());

        assert_eq!(config.file_for(Path::new("/definitely/not/mapped")), None);
    }

    #[test]
    fn duplicate_folders_are_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret(
            secret.path(),
            &[("first", source.path()), ("second", source.path())],
        );
        write_ini(secret.path(), "first");
        write_ini(secret.path(), "second");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("the duplicate folder must fail");

        assert!(
            matches!(error, ConfigError::DuplicateFolder { .. }),
            "{error}"
        );
    }

    #[test]
    fn invalid_names_and_relative_folders_are_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret(secret.path(), &[("../escape", source.path())]);
        write_ini(secret.path(), "escape");
        let error =
            ConfidentialConfig::from_public_file(&public).expect_err("the invalid name must fail");
        assert!(
            matches!(error, ConfigError::InvalidIniName { .. }),
            "{error}"
        );

        write_secret(secret.path(), &[("relative", Path::new("relative"))]);
        write_ini(secret.path(), "relative");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("the relative folder must fail");
        assert!(matches!(error, ConfigError::NotAbsolute { .. }), "{error}");
    }

    #[test]
    fn a_missing_ini_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret(secret.path(), &[("escritorio", source.path())]);
        let error =
            ConfidentialConfig::from_public_file(&public).expect_err("the missing INI must fail");

        assert!(matches!(error, ConfigError::IniRead { .. }), "{error}");
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let public = temp.path().join(PUBLIC_FILE_NAME);
        fs::write(&public, "version: 2\nconfidential_directories: []\n")
            .expect("the configuration must be written");
        let error =
            ConfidentialConfig::from_public_file(&public).expect_err("the version must fail");

        assert!(matches!(error, ConfigError::Version { .. }), "{error}");
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let public = temp.path().join(PUBLIC_FILE_NAME);
        fs::write(
            &public,
            "version: 1\nconfidential_directories: []\nunknown: true\n",
        )
        .expect("the configuration must be written");
        let error =
            ConfidentialConfig::from_public_file(&public).expect_err("the unknown key must fail");

        assert!(matches!(error, ConfigError::PublicParse { .. }), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_are_required() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret(secret.path(), &[("escritorio", source.path())]);
        write_ini(secret.path(), "escritorio");
        set_mode(&secret.path().join(SECRET_FILE_NAME), 0o644);
        let error =
            ConfidentialConfig::from_public_file(&public).expect_err("the readable map must fail");
        assert!(
            matches!(error, ConfigError::FilePermissions { .. }),
            "{error}"
        );

        write_secret(secret.path(), &[("escritorio", source.path())]);
        set_mode(secret.path(), 0o755);
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("the readable directory must fail");
        assert!(
            matches!(error, ConfigError::DirectoryPermissions { .. }),
            "{error}"
        );
    }
}
