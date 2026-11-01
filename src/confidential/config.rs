//! The public user configuration and the confidential map.
//!
//! The public `config.yaml` holds the default output directory and lists the
//! confidential directories. Each directory owns a `confidential.yaml` that
//! names its INI files, the source folders they serve and optional outputs.
//! Keeping the mapping inside the confidential directory keeps the project
//! names out of the general configuration.

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

/// One source folder, the INI file that serves it and an optional output.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    source: PathBuf,
    ini: PathBuf,
    output: Option<PathBuf>,
}

/// The public user configuration: the default output and the confidential
/// directories.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserConfig {
    default_output: Option<PathBuf>,
    confidential_directories: Vec<PathBuf>,
}

impl UserConfig {
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

        let default_output = match file.default_output {
            Some(text) => Some(absolute_output(&text)?),
            None => None,
        };
        Ok(Self {
            default_output,
            confidential_directories: file
                .confidential_directories
                .into_iter()
                .map(PathBuf::from)
                .collect(),
        })
    }

    /// The output directory used when the command line and the INI mapping do
    /// not choose one.
    pub fn default_output(&self) -> Option<&Path> {
        self.default_output.as_deref()
    }

    /// Builds the resolved folder-to-INI map.
    pub fn confidential_config(&self) -> Result<ConfidentialConfig, ConfigError> {
        ConfidentialConfig::from_directories(&self.confidential_directories)
    }
}

/// The resolved folder-to-INI map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfidentialConfig {
    entries: Vec<Entry>,
}

impl ConfidentialConfig {
    /// Loads the confidential map from the user configuration directory.
    ///
    /// A missing public file is an empty configuration, not an error.
    pub fn load_default() -> Result<Self, ConfigError> {
        UserConfig::load_default()?.confidential_config()
    }

    /// Loads a confidential map from an explicit public configuration path.
    pub fn from_public_file(path: &Path) -> Result<Self, ConfigError> {
        UserConfig::from_public_file(path)?.confidential_config()
    }

    fn from_directories(directories: &[PathBuf]) -> Result<Self, ConfigError> {
        let mut seen = BTreeSet::new();
        let mut entries = Vec::new();
        for directory in directories {
            if !directory.is_absolute() {
                return Err(ConfigError::NotAbsolute {
                    path: directory.clone(),
                });
            }
            let normalized =
                normalize_absolute(directory).ok_or_else(|| ConfigError::NotAbsolute {
                    path: directory.clone(),
                })?;
            let metadata =
                fs::metadata(&normalized).map_err(|source| ConfigError::DirectoryRead {
                    path: normalized.clone(),
                    source,
                })?;
            if !metadata.is_dir() {
                return Err(ConfigError::NotDirectory { path: normalized });
            }
            check_directory_permissions(&normalized, &metadata)?;
            if !seen.insert(normalized.clone()) {
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
        self.entry_for(source_root).map(|entry| entry.ini.as_path())
    }

    /// The output directory mapped to a source folder, using the longest
    /// matching prefix. A more specific entry without an output hides a broader
    /// entry's output.
    pub fn output_for(&self, source_root: &Path) -> Option<&Path> {
        self.entry_for(source_root)
            .and_then(|entry| entry.output.as_deref())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn entry_for(&self, source_root: &Path) -> Option<&Entry> {
        let root = normalize_absolute(source_root)?;
        self.entries
            .iter()
            .find(|entry| root.starts_with(&entry.source))
    }
}

fn absolute_output(text: &str) -> Result<PathBuf, ConfigError> {
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(ConfigError::NotAbsolute { path });
    }
    normalize_absolute(&path).ok_or(ConfigError::NotAbsolute { path })
}

/// Where the resolved output directory came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputOrigin {
    Explicit,
    Ini,
    Default,
    Source,
}

/// The output directory chosen for a source root and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOutput {
    directory: Option<PathBuf>,
    origin: OutputOrigin,
}

impl ResolvedOutput {
    /// The directory, or `None` when the output stays beside the source.
    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    pub fn origin(&self) -> OutputOrigin {
        self.origin
    }
}

/// Resolves the output directory with the shared precedence: explicit,
/// per-INI output, user default and finally the source folder.
pub fn resolve_output(
    source_root: &Path,
    explicit: Option<&Path>,
    user: &UserConfig,
    confidential: Option<&ConfidentialConfig>,
) -> ResolvedOutput {
    if let Some(directory) = explicit {
        return ResolvedOutput {
            directory: Some(directory.to_owned()),
            origin: OutputOrigin::Explicit,
        };
    }
    if let Some(directory) = confidential.and_then(|config| config.output_for(source_root)) {
        return ResolvedOutput {
            directory: Some(directory.to_owned()),
            origin: OutputOrigin::Ini,
        };
    }
    if let Some(directory) = user.default_output() {
        return ResolvedOutput {
            directory: Some(directory.to_owned()),
            origin: OutputOrigin::Default,
        };
    }
    ResolvedOutput {
        directory: None,
        origin: OutputOrigin::Source,
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicFile {
    version: u32,
    #[serde(default)]
    default_output: Option<String>,
    #[serde(default)]
    confidential_directories: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretFile {
    version: u32,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    outputs: BTreeMap<String, String>,
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

    let SecretFile { files, outputs, .. } = file;
    let mut entries = Vec::with_capacity(files.len());
    for (name, folder) in files {
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
        entries.push(Entry {
            source,
            ini,
            output: None,
        });
    }

    let mut seen_outputs = BTreeSet::new();
    for (name, output) in outputs {
        let name = IniName::parse(&name)
            .ok_or_else(|| ConfigError::InvalidIniName { name: name.clone() })?;
        let ini_name = name.file_name();
        if !seen_outputs.insert(ini_name.to_owned()) {
            return Err(ConfigError::DuplicateOutput {
                name: ini_name.to_owned(),
            });
        }
        let output = absolute_output(&output)?;
        let mut matched = false;
        for entry in entries.iter_mut().filter(|entry| {
            entry
                .ini
                .file_name()
                .and_then(|file_name| file_name.to_str())
                == Some(ini_name)
        }) {
            if super::is_within(&entry.source, &output) {
                return Err(ConfigError::OutputInsideSource {
                    output,
                    source: entry.source.clone(),
                });
            }
            entry.output = Some(output.clone());
            matched = true;
        }
        if !matched {
            return Err(ConfigError::UnknownOutput {
                name: ini_name.to_owned(),
            });
        }
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
#[non_exhaustive]
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
    UnknownOutput {
        name: String,
    },
    DuplicateOutput {
        name: String,
    },
    OutputInsideSource {
        output: PathBuf,
        source: PathBuf,
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
            Self::UnknownOutput { name } => write!(
                formatter,
                "the confidential output for `{name}` has no matching entry in files"
            ),
            Self::DuplicateOutput { name } => write!(
                formatter,
                "the confidential output for `{name}` is listed more than once"
            ),
            Self::OutputInsideSource { output, source } => write!(
                formatter,
                "the confidential output {} must be outside the source folder {}",
                output.display(),
                source.display()
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
            | Self::DuplicateFolder { .. }
            | Self::UnknownOutput { .. }
            | Self::DuplicateOutput { .. }
            | Self::OutputInsideSource { .. } => None,
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
        write_public_with(path, directories, None);
    }

    fn write_public_with(path: &Path, directories: &[&Path], default_output: Option<&Path>) {
        let mut text = String::from("version: 1\n");
        if let Some(output) = default_output {
            text.push_str(&format!(
                "default_output: {}\n",
                yaml_string(&output.display().to_string())
            ));
        }
        if !directories.is_empty() {
            text.push_str("confidential_directories:\n");
            for directory in directories {
                text.push_str(&format!(
                    "  - {}\n",
                    yaml_string(&directory.display().to_string())
                ));
            }
        }
        fs::write(path, text).expect("the public configuration must be written");
    }

    fn write_secret(directory: &Path, files: &[(&str, &Path)]) {
        write_secret_with(directory, files, &[]);
    }

    fn write_secret_with(directory: &Path, files: &[(&str, &Path)], outputs: &[(&str, &Path)]) {
        let mut text = String::from("version: 1\nfiles:\n");
        for (name, folder) in files {
            text.push_str(&format!(
                "  {}: {}\n",
                yaml_string(name),
                yaml_string(&folder.display().to_string())
            ));
        }
        if !outputs.is_empty() {
            text.push_str("outputs:\n");
            for (name, output) in outputs {
                text.push_str(&format!(
                    "  {}: {}\n",
                    yaml_string(name),
                    yaml_string(&output.display().to_string())
                ));
            }
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
    fn a_default_output_is_loaded() {
        let temp = TempDir::new().expect("a configuration directory");
        let output = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public_with(&public, &[], Some(output.path()));
        let config =
            UserConfig::from_public_file(&public).expect("the public configuration must load");

        assert_eq!(
            config.default_output(),
            Some(resolved(output.path()).as_path())
        );
        assert!(
            config
                .confidential_config()
                .expect("an empty mapping")
                .is_empty()
        );
    }

    #[test]
    fn the_output_precedence_is_explicit_ini_default_source() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let ini_output = project();
        let default_output = project();
        let explicit = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public_with(&public, &[secret.path()], Some(default_output.path()));
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[("escritorio", ini_output.path())],
        );
        write_ini(secret.path(), "escritorio");
        let user =
            UserConfig::from_public_file(&public).expect("the public configuration must load");
        let confidential = user
            .confidential_config()
            .expect("the confidential mapping must load");

        let choice = resolve_output(
            source.path(),
            Some(explicit.path()),
            &user,
            Some(&confidential),
        );
        assert_eq!(choice.origin(), OutputOrigin::Explicit);
        assert_eq!(choice.directory(), Some(explicit.path()));

        let choice = resolve_output(source.path(), None, &user, Some(&confidential));
        assert_eq!(choice.origin(), OutputOrigin::Ini);
        assert_eq!(
            choice.directory(),
            Some(resolved(ini_output.path()).as_path())
        );

        let choice = resolve_output(source.path(), None, &user, None);
        assert_eq!(choice.origin(), OutputOrigin::Default);
        assert_eq!(
            choice.directory(),
            Some(resolved(default_output.path()).as_path())
        );

        let empty = UserConfig::default();
        let choice = resolve_output(source.path(), None, &empty, None);
        assert_eq!(choice.origin(), OutputOrigin::Source);
        assert_eq!(choice.directory(), None);
    }

    #[test]
    fn a_relative_default_output_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let public = temp.path().join(PUBLIC_FILE_NAME);
        fs::write(&public, "version: 1\ndefault_output: \"relative/out\"\n")
            .expect("the public configuration must be written");
        let error =
            UserConfig::from_public_file(&public).expect_err("the relative output must fail");

        assert!(matches!(error, ConfigError::NotAbsolute { .. }), "{error}");
    }

    #[test]
    fn an_output_is_resolved_for_its_ini() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let output = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[("escritorio", output.path())],
        );
        write_ini(secret.path(), "escritorio");
        let config =
            ConfidentialConfig::from_public_file(&public).expect("the configuration must load");

        let expected = resolved(output.path());
        assert_eq!(config.output_for(source.path()), Some(expected.as_path()));
        assert_eq!(
            config.output_for(&source.path().join("nested")),
            Some(expected.as_path())
        );
    }

    #[test]
    fn a_specific_entry_without_an_output_hides_the_broader_output() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let nested = source.path().join("nested");
        fs::create_dir(&nested).expect("the nested folder must exist");
        let output = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("outside", source.path()), ("inside", &nested)],
            &[("outside", output.path())],
        );
        write_ini(secret.path(), "outside");
        write_ini(secret.path(), "inside");
        let config =
            ConfidentialConfig::from_public_file(&public).expect("the configuration must load");

        assert_eq!(config.output_for(&nested), None);
        assert_eq!(
            config.output_for(source.path()),
            Some(resolved(output.path()).as_path())
        );
    }

    #[test]
    fn an_output_inside_its_source_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let inside = source.path().join("out");
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[("escritorio", inside.as_path())],
        );
        write_ini(secret.path(), "escritorio");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("an output inside the source must fail");

        assert!(
            matches!(error, ConfigError::OutputInsideSource { .. }),
            "{error}"
        );
    }

    #[test]
    fn an_output_without_a_matching_ini_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let output = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[("missing", output.path())],
        );
        write_ini(secret.path(), "escritorio");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("an output without an INI must fail");

        assert!(
            matches!(error, ConfigError::UnknownOutput { .. }),
            "{error}"
        );
    }

    #[test]
    fn a_relative_ini_output_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[("escritorio", Path::new("relative/out"))],
        );
        write_ini(secret.path(), "escritorio");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("the relative output must fail");

        assert!(matches!(error, ConfigError::NotAbsolute { .. }), "{error}");
    }

    #[test]
    fn an_output_key_with_a_path_separator_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let output = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[("dir/out", output.path())],
        );
        write_ini(secret.path(), "escritorio");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("an invalid output key must fail");

        assert!(
            matches!(error, ConfigError::InvalidIniName { .. }),
            "{error}"
        );
    }

    #[test]
    fn a_duplicate_output_key_is_rejected() {
        let temp = TempDir::new().expect("a configuration directory");
        let secret = secret_directory();
        let source = project();
        let first = project();
        let second = project();
        let public = temp.path().join(PUBLIC_FILE_NAME);
        write_public(&public, &[secret.path()]);
        write_secret_with(
            secret.path(),
            &[("escritorio", source.path())],
            &[
                ("escritorio", first.path()),
                ("escritorio.ini", second.path()),
            ],
        );
        write_ini(secret.path(), "escritorio");
        let error = ConfidentialConfig::from_public_file(&public)
            .expect_err("a duplicate output key must fail");

        assert!(
            matches!(error, ConfigError::DuplicateOutput { .. }),
            "{error}"
        );
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
