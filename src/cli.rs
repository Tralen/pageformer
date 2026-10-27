//! Command-line helpers: input discovery, output planning and terminal styling.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The output formats the command can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pdf,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
        }
    }
}

/// A Markdown file and the root that discovered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFile {
    path: PathBuf,
    root: PathBuf,
}

impl InputFile {
    pub fn new(path: PathBuf, root: PathBuf) -> Self {
        Self { path, root }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The source tree that discovered this file.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Computes the output path for this input.
    ///
    /// A directory root keeps the relative tree under the output directory; a
    /// direct file is written flat.
    pub fn output_path(&self, directory: Option<&Path>, format: OutputFormat) -> Result<PathBuf> {
        match directory {
            None => Ok(self.path.with_extension(format.extension())),
            Some(directory) => {
                let relative = self.relative_path()?;
                Ok(directory.join(relative).with_extension(format.extension()))
            }
        }
    }

    fn relative_path(&self) -> Result<PathBuf> {
        if let Ok(relative) = self.path.strip_prefix(&self.root)
            && !relative.as_os_str().is_empty()
        {
            return Ok(relative.to_path_buf());
        }
        self.path
            .file_name()
            .map(PathBuf::from)
            .with_context(|| format!("{} has no file name", self.path.display()))
    }
}

/// A planned input and its output path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedInput {
    pub input: InputFile,
    pub output: PathBuf,
}

impl PlannedInput {
    pub fn into_parts(self) -> (InputFile, PathBuf) {
        (self.input, self.output)
    }
}

/// A planning failure that is not tied to reading the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    pub input: PathBuf,
    pub message: String,
}

impl PlanError {
    pub fn new(input: PathBuf, message: String) -> Self {
        Self { input, message }
    }
}

/// Plans output paths and rejects collisions before rendering starts.
pub fn plan_outputs(
    files: Vec<InputFile>,
    directory: Option<&Path>,
    format: OutputFormat,
) -> (Vec<PlannedInput>, Vec<PlanError>) {
    let mut planned = Vec::new();
    let mut errors = Vec::new();
    let mut seen: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    for input in files {
        match input.output_path(directory, format) {
            Ok(output) => match seen.get(&output) {
                Some(first) => errors.push(PlanError::new(
                    input.path().to_path_buf(),
                    format!(
                        "output {} is already used by {}",
                        output.display(),
                        first.display()
                    ),
                )),
                None => {
                    seen.insert(output.clone(), input.path().to_path_buf());
                    planned.push(PlannedInput { input, output });
                }
            },
            Err(error) => errors.push(PlanError::new(
                input.path().to_path_buf(),
                format!("{error:#}"),
            )),
        }
    }
    (planned, errors)
}

/// A warning found while discovering inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryWarning {
    pub path: PathBuf,
    pub message: String,
}

impl DiscoveryWarning {
    pub fn new(path: PathBuf, message: String) -> Self {
        Self { path, message }
    }
}

/// The Markdown files found under one input and the warnings raised on the way.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub files: Vec<InputFile>,
    pub warnings: Vec<DiscoveryWarning>,
}

/// Why an input could not be discovered.
#[derive(Debug)]
pub enum DiscoveryError {
    Metadata { path: PathBuf, source: io::Error },
    ReadDirectory { path: PathBuf, source: io::Error },
    Unsupported { path: PathBuf },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::ReadDirectory { path, source } => {
                write!(
                    formatter,
                    "cannot read directory {}: {source}",
                    path.display()
                )
            }
            Self::Unsupported { path } => {
                write!(
                    formatter,
                    "{} is neither a file nor a directory",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Metadata { source, .. } | Self::ReadDirectory { source, .. } => Some(source),
            Self::Unsupported { .. } => None,
        }
    }
}

/// Discovers a file directly or every Markdown file under a directory.
pub fn discover(input: &Path) -> Result<Discovered, DiscoveryError> {
    let metadata = fs::metadata(input).map_err(|source| DiscoveryError::Metadata {
        path: input.to_path_buf(),
        source,
    })?;
    if metadata.is_file() {
        return Ok(Discovered {
            files: vec![InputFile::new(
                input.to_path_buf(),
                parent_or_current(input),
            )],
            warnings: Vec::new(),
        });
    }
    if metadata.is_dir() {
        let mut discovered = Discovered::default();
        walk(input, input, &mut discovered)?;
        if discovered.files.is_empty() {
            discovered.warnings.push(DiscoveryWarning::new(
                input.to_path_buf(),
                format!("no Markdown files found in {}", input.display()),
            ));
        }
        discovered
            .files
            .sort_by(|left, right| left.path().cmp(right.path()));
        discovered.warnings.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.message.cmp(&right.message))
        });
        return Ok(discovered);
    }
    Err(DiscoveryError::Unsupported {
        path: input.to_path_buf(),
    })
}

fn parent_or_current(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn walk(root: &Path, directory: &Path, discovered: &mut Discovered) -> Result<(), DiscoveryError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if directory == root => {
            return Err(DiscoveryError::ReadDirectory {
                path: directory.to_path_buf(),
                source,
            });
        }
        Err(source) => {
            discovered.warnings.push(DiscoveryWarning::new(
                directory.to_path_buf(),
                format!("cannot read directory {}: {source}", directory.display()),
            ));
            return Ok(());
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                discovered.warnings.push(DiscoveryWarning::new(
                    directory.to_path_buf(),
                    format!("cannot read an entry in {}: {source}", directory.display()),
                ));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(source) => {
                let message = format!("cannot inspect {}: {source}", path.display());
                discovered
                    .warnings
                    .push(DiscoveryWarning::new(path, message));
                continue;
            }
        };
        if file_type.is_dir() {
            if is_skipped_directory(&entry.file_name()) {
                continue;
            }
            walk(root, &path, discovered)?;
        } else if file_type.is_file() {
            if is_markdown(&path) {
                discovered
                    .files
                    .push(InputFile::new(path, root.to_path_buf()));
            }
        } else if file_type.is_symlink() {
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(metadata) if metadata.is_file() && is_markdown(&path) => {
                    discovered
                        .files
                        .push(InputFile::new(path, root.to_path_buf()));
                }
                Ok(_) => {}
                Err(source) => {
                    let message = format!("cannot inspect {}: {source}", path.display());
                    discovered
                        .warnings
                        .push(DiscoveryWarning::new(path, message));
                }
            }
        }
    }
    Ok(())
}

fn is_skipped_directory(name: &OsStr) -> bool {
    name == OsStr::new("target") || name.to_string_lossy().starts_with('.')
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == OsStr::new("md"))
}

/// Writes a file through a temporary file in the same directory.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let temporary = temporary_path(path)?;
    if let Err(error) = fs::write(&temporary, bytes) {
        let _ = fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("cannot write {}", temporary.display()));
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("cannot replace {}", path.display()));
    }
    Ok(())
}

fn temporary_path(path: &Path) -> Result<PathBuf> {
    let mut name = path
        .file_name()
        .with_context(|| format!("{} has no file name", path.display()))?
        .to_os_string();
    name.push(".tmp");
    Ok(path.with_file_name(name))
}

/// How the command decides whether to color its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }

    pub fn enabled(self, stream: Stream) -> bool {
        self.resolve(
            stream.is_terminal(),
            env::var_os("NO_COLOR").is_some(),
            env::var_os("TERM").is_some_and(|term| term.to_string_lossy() == "dumb"),
        )
    }

    pub fn resolve(self, is_terminal: bool, no_color: bool, term_dumb: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => is_terminal && !no_color && !term_dumb,
        }
    }
}

/// The stream whose terminal capability is being checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    fn is_terminal(self) -> bool {
        match self {
            Self::Stdout => io::stdout().is_terminal(),
            Self::Stderr => io::stderr().is_terminal(),
        }
    }
}

/// A terminal style applied to a piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Error,
    Warning,
    Success,
    Info,
    Path,
    Dim,
    Italic,
}

impl Style {
    fn open(self) -> &'static str {
        match self {
            Self::Error => "\x1b[1;31m",
            Self::Warning => "\x1b[1;33m",
            Self::Success => "\x1b[1;32m",
            Self::Info => "\x1b[1;36m",
            Self::Path => "\x1b[1m",
            Self::Dim => "\x1b[2m",
            Self::Italic => "\x1b[3m",
        }
    }
}

const RESET: &str = "\x1b[0m";

/// Applies ANSI styles when the target stream accepts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Painter {
    enabled: bool,
}

impl Painter {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn render(self, style: Style, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", style.open(), text, RESET)
        } else {
            text.to_owned()
        }
    }

    pub fn path(self, path: &Path) -> String {
        self.render(Style::Path, &path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(path: &Path, contents: &str) {
        fs::write(path, contents).expect("the fixture must be written");
    }

    #[test]
    fn a_direct_file_is_an_input() {
        let project = TempDir::new().expect("a temporary project");
        let path = project.path().join("A.md");
        write_file(&path, "# A\n");

        let discovered = discover(&path).expect("the file must be discovered");

        assert_eq!(discovered.files.len(), 1);
        assert_eq!(discovered.files[0].path(), path);
        assert!(discovered.warnings.is_empty());
    }

    #[test]
    fn a_directory_is_walked_in_path_order_and_skips_hidden_and_target() {
        let project = TempDir::new().expect("a temporary project");
        let root = project.path();
        fs::create_dir_all(root.join("sub")).expect("the subdirectory must be created");
        fs::create_dir_all(root.join(".hidden")).expect("the hidden directory must be created");
        fs::create_dir_all(root.join("sub/target")).expect("the target directory must be created");
        write_file(&root.join("B.md"), "# B\n");
        write_file(&root.join("A.md"), "# A\n");
        write_file(&root.join("sub/C.md"), "# C\n");
        write_file(&root.join(".hidden/D.md"), "# D\n");
        write_file(&root.join("sub/target/E.md"), "# E\n");
        write_file(&root.join("notes.txt"), "text");

        let discovered = discover(root).expect("the directory must be discovered");
        let names = discovered
            .files
            .iter()
            .map(|file| {
                file.path()
                    .strip_prefix(root)
                    .expect("the file must be under the root")
                    .to_path_buf()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                PathBuf::from("A.md"),
                PathBuf::from("B.md"),
                PathBuf::from("sub/C.md"),
            ]
        );
        assert!(discovered.warnings.is_empty());
    }

    #[test]
    fn an_empty_directory_is_a_warning() {
        let project = TempDir::new().expect("a temporary project");

        let discovered = discover(project.path()).expect("the directory must be discovered");

        assert!(discovered.files.is_empty());
        assert_eq!(discovered.warnings.len(), 1);
        assert!(discovered.warnings[0].message.contains("no Markdown files"));
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_symlink_is_not_followed() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("a temporary project");
        let root = project.path();
        fs::create_dir_all(root.join("real")).expect("the real directory must be created");
        write_file(&root.join("real/A.md"), "# A\n");
        symlink(root.join("real"), root.join("link")).expect("the symlink must be created");

        let discovered = discover(root).expect("the directory must be discovered");
        let names = discovered
            .files
            .iter()
            .map(|file| {
                file.path()
                    .strip_prefix(root)
                    .expect("the file must be under the root")
                    .to_path_buf()
            })
            .collect::<Vec<_>>();

        assert_eq!(names, vec![PathBuf::from("real/A.md")]);
    }

    #[test]
    fn a_directory_input_mirrors_its_tree_under_the_output() {
        let input = InputFile::new(PathBuf::from("docs/sub/A.md"), PathBuf::from("docs"));

        let output = input
            .output_path(Some(Path::new("out")), OutputFormat::Pdf)
            .expect("the output path must be computed");

        assert_eq!(output, PathBuf::from("out/sub/A.pdf"));
    }

    #[test]
    fn a_direct_file_is_written_flat_under_the_output() {
        let input = InputFile::new(PathBuf::from("docs/A.md"), PathBuf::from("docs"));

        let output = input
            .output_path(Some(Path::new("out")), OutputFormat::Pdf)
            .expect("the output path must be computed");

        assert_eq!(output, PathBuf::from("out/A.pdf"));
    }

    #[test]
    fn without_an_output_directory_the_file_stays_beside_its_source() {
        let input = InputFile::new(PathBuf::from("docs/A.md"), PathBuf::from("docs"));

        let output = input
            .output_path(None, OutputFormat::Pdf)
            .expect("the output path must be computed");

        assert_eq!(output, PathBuf::from("docs/A.pdf"));
    }

    #[test]
    fn two_inputs_cannot_share_an_output() {
        let files = vec![
            InputFile::new(PathBuf::from("one/A.md"), PathBuf::from("one")),
            InputFile::new(PathBuf::from("two/A.md"), PathBuf::from("two")),
        ];

        let (planned, errors) = plan_outputs(files, Some(Path::new("out")), OutputFormat::Pdf);

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].output, PathBuf::from("out/A.pdf"));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("already used"));
    }

    #[test]
    fn an_atomic_write_creates_the_parent_and_removes_the_temporary_file() {
        let project = TempDir::new().expect("a temporary project");
        let path = project.path().join("out/A.pdf");

        write_atomic(&path, b"new").expect("the file must be written");

        assert_eq!(fs::read(&path).expect("the file must exist"), b"new");
        assert!(!path.with_file_name("A.pdf.tmp").exists());
    }

    #[test]
    fn an_enabled_painter_emits_ansi() {
        assert_eq!(
            Painter::new(true).render(Style::Error, "error"),
            "\x1b[1;31merror\x1b[0m"
        );
    }

    #[test]
    fn a_disabled_painter_emits_plain_text() {
        assert_eq!(Painter::new(false).render(Style::Error, "error"), "error");
    }

    #[test]
    fn auto_color_needs_a_terminal_and_no_opt_out() {
        assert!(ColorChoice::Auto.resolve(true, false, false));
        assert!(!ColorChoice::Auto.resolve(false, false, false));
        assert!(!ColorChoice::Auto.resolve(true, true, false));
        assert!(!ColorChoice::Auto.resolve(true, false, true));
    }

    #[test]
    fn forced_and_disabled_colors_ignore_the_environment() {
        assert!(ColorChoice::Always.resolve(false, true, true));
        assert!(!ColorChoice::Never.resolve(true, false, false));
    }

    #[test]
    fn the_three_color_arguments_are_recognized() {
        assert_eq!(ColorChoice::parse("auto"), Some(ColorChoice::Auto));
        assert_eq!(ColorChoice::parse("always"), Some(ColorChoice::Always));
        assert_eq!(ColorChoice::parse("never"), Some(ColorChoice::Never));
        assert_eq!(ColorChoice::parse("sometimes"), None);
    }
}
