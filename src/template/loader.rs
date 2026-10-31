//! Finds, reads and validates the template that applies to a source document.
//!
//! The nearest local template wins entirely. The global template in the user
//! configuration directory is the second choice, and the embedded fallback is the
//! last one. An invalid template never falls back to another one.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Template, TemplateError};

/// The hidden template name used inside a project directory.
pub const LOCAL_FILE_NAME: &str = ".pageformer.yaml";

/// The global template name inside the user configuration directory.
pub const GLOBAL_FILE_NAME: &str = "pageformer.yaml";

/// The directory name used under the user configuration directory.
pub const GLOBAL_DIRECTORY_NAME: &str = crate::config::DIRECTORY_NAME;

/// A template together with the place it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTemplate {
    origin: TemplateOrigin,
    template: Template,
}

impl ResolvedTemplate {
    pub fn new(origin: TemplateOrigin, template: Template) -> Self {
        Self { origin, template }
    }

    pub fn origin(&self) -> &TemplateOrigin {
        &self.origin
    }

    pub fn template(&self) -> &Template {
        &self.template
    }

    /// The directory that relative paths in the template resolve against.
    pub fn base_directory(&self) -> Option<&Path> {
        self.origin.path().and_then(Path::parent)
    }
}

/// Where a resolved template came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplateOrigin {
    Local(PathBuf),
    Global(PathBuf),
    Explicit(PathBuf),
    Embedded,
}

impl TemplateOrigin {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Local(path) | Self::Global(path) | Self::Explicit(path) => Some(path),
            Self::Embedded => None,
        }
    }
}

impl fmt::Display for TemplateOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(path) | Self::Global(path) | Self::Explicit(path) => {
                write!(formatter, "{}", path.display())
            }
            Self::Embedded => formatter.write_str("embedded"),
        }
    }
}

/// Why a template could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: TemplateError,
    },
    Embedded {
        source: TemplateError,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "cannot read template {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, source } => {
                write!(formatter, "template {}: {source}", path.display())
            }
            Self::Embedded { source } => {
                write!(formatter, "the embedded template is invalid: {source}")
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } | Self::Embedded { source } => Some(source),
        }
    }
}

/// The local template paths that may apply, nearest first.
pub fn local_candidates(source_directory: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    source_directory
        .ancestors()
        .map(|directory| directory.join(LOCAL_FILE_NAME))
}

/// The nearest local template that exists, if any.
pub fn find_local(source_directory: &Path) -> Option<PathBuf> {
    local_candidates(source_directory).find(|candidate| candidate.is_file())
}

/// Reads and validates a template file.
pub fn load_file(path: &Path) -> Result<Template, LoadError> {
    let text = fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_owned(),
        source,
    })?;
    Template::from_yaml(&text).map_err(|source| LoadError::Parse {
        path: path.to_owned(),
        source,
    })
}

/// Resolves the template for a document directory.
///
/// `global` is the explicit global template path, usually from
/// [`global_template_path`]; it is used only when no local template exists.
pub fn resolve(
    source_directory: &Path,
    global: Option<&Path>,
) -> Result<ResolvedTemplate, LoadError> {
    if let Some(path) = find_local(source_directory) {
        let template = load_file(&path)?;
        return Ok(ResolvedTemplate::new(TemplateOrigin::Local(path), template));
    }
    match global.filter(|path| path.exists()) {
        Some(path) => {
            let template = load_file(path)?;
            Ok(ResolvedTemplate::new(
                TemplateOrigin::Global(path.to_owned()),
                template,
            ))
        }
        None => Template::embedded()
            .map(|template| ResolvedTemplate::new(TemplateOrigin::Embedded, template))
            .map_err(|source| LoadError::Embedded { source }),
    }
}

/// The global template path in the user configuration directory.
pub fn global_template_path() -> Option<PathBuf> {
    crate::config::user_config_directory().map(|directory| directory.join(GLOBAL_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_candidates_are_ordered_nearest_first() {
        let candidates = local_candidates(Path::new("/project/docs/legal"))
            .take(3)
            .collect::<Vec<_>>();
        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/project/docs/legal/.pageformer.yaml"),
                PathBuf::from("/project/docs/.pageformer.yaml"),
                PathBuf::from("/project/.pageformer.yaml"),
            ]
        );
    }

    #[test]
    fn embedded_origin_has_no_path() {
        assert_eq!(TemplateOrigin::Embedded.path(), None);
        assert_eq!(TemplateOrigin::Embedded.to_string(), "embedded");
    }
}
