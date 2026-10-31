//! The effectful document pipeline: read, parse, resolve, render and write.
//!
//! The pure pieces live in `markdown`, `document`, `template`, `assets` and the
//! renderers. This module is the smallest effectful edge: it reads the source,
//! resolves the template and the confidential fields, calls one backend and
//! either returns the bytes (`render`) or writes the output (`export`). The CLI,
//! the minimal GUI and Pageweaver share it.

use std::borrow::Cow;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::confidential::{self, ConfidentialStore, ResolveError};
use crate::document::Document;
use crate::docx::{DocxError, DocxRenderer};
use crate::format::Format;
use crate::markdown::{self, MarkdownError};
use crate::odt::{OdtError, OdtRenderer};
use crate::pdf::{self, PdfError, PdfRenderer};
use crate::render::{Rendered, Renderer};
use crate::template::loader::{self, LoadError, ResolvedTemplate, TemplateOrigin};
use crate::txt::TxtRenderer;

/// The Markdown source of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A file read from disk.
    File(PathBuf),
    /// An in-memory buffer with the path used to resolve the template and the
    /// assets. The file does not need to exist.
    Buffer { text: String, path: PathBuf },
}

impl Source {
    /// The path used for template and asset resolution and for the confidential
    /// output rule.
    pub fn path(&self) -> &Path {
        match self {
            Self::File(path) | Self::Buffer { path, .. } => path,
        }
    }

    fn text(&self) -> Result<Cow<'_, str>, ExportError> {
        match self {
            Self::File(path) => {
                fs::read_to_string(path)
                    .map(Cow::Owned)
                    .map_err(|source| ExportError::Read {
                        path: path.clone(),
                        source,
                    })
            }
            Self::Buffer { text, .. } => Ok(Cow::Borrowed(text.as_str())),
        }
    }
}

/// A document to check or export, before the output path is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentJob {
    source: Source,
    source_root: PathBuf,
    template: Option<PathBuf>,
    global_template: Option<PathBuf>,
}

impl DocumentJob {
    /// Builds a job from a source and the tree that owns it.
    pub fn new(source: Source, source_root: impl Into<PathBuf>) -> Self {
        Self {
            source,
            source_root: source_root.into(),
            template: None,
            global_template: None,
        }
    }

    /// Uses an explicit template instead of resolving one.
    pub fn with_template(mut self, template: Option<PathBuf>) -> Self {
        self.template = template;
        self
    }

    /// The global template used when no local or explicit template exists.
    pub fn with_global_template(mut self, global: Option<PathBuf>) -> Self {
        self.global_template = global;
        self
    }

    /// The source this job reads.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The path used for template and asset resolution.
    pub fn source_path(&self) -> &Path {
        self.source.path()
    }

    /// The tree that owns the source, used by the confidential output rule.
    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
}

/// The result of validating a document and its template without writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    blocks: usize,
    headings: usize,
    template: String,
    confidential: Vec<String>,
}

impl CheckOutcome {
    /// The number of blocks in the parsed document.
    pub fn blocks(&self) -> usize {
        self.blocks
    }

    /// The number of headings in the parsed document.
    pub fn headings(&self) -> usize {
        self.headings
    }

    /// The template that was resolved, as a display string.
    pub fn template(&self) -> &str {
        &self.template
    }

    /// The confidential field names found, without their values.
    pub fn confidential(&self) -> &[String] {
        &self.confidential
    }
}

/// A written output and the warnings the backend reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutcome {
    output: PathBuf,
    warnings: Vec<String>,
}

/// The rendered bytes of a document and the warnings the backend reported,
/// before an output path is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutcome {
    bytes: Vec<u8>,
    warnings: Vec<String>,
}

impl RenderOutcome {
    /// The rendered bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The warnings the backend reported.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

impl ExportOutcome {
    /// The path that was written.
    pub fn output(&self) -> &Path {
        &self.output
    }

    /// The warnings the backend reported.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// Why a document could not be checked or exported.
#[derive(Debug)]
pub enum ExportError {
    Read { path: PathBuf, source: io::Error },
    Markdown(MarkdownError),
    Confidential(ResolveError),
    Template(LoadError),
    Pdf(PdfError),
    Odt(OdtError),
    Docx(DocxError),
    ConfidentialOutput { root: PathBuf },
    OutputPath { path: PathBuf },
    CreateDirectory { path: PathBuf, source: io::Error },
    Write { path: PathBuf, source: io::Error },
    Replace { path: PathBuf, source: io::Error },
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::Markdown(error) => write!(formatter, "cannot parse the Markdown: {error}"),
            Self::Confidential(error) => {
                write!(formatter, "cannot resolve confidential fields: {error}")
            }
            Self::Template(error) => write!(formatter, "{error}"),
            Self::Pdf(error) => write!(formatter, "cannot render the PDF: {error}"),
            Self::Odt(error) => write!(formatter, "cannot render the ODT: {error}"),
            Self::Docx(error) => write!(formatter, "cannot render the DOCX: {error}"),
            Self::ConfidentialOutput { root } => write!(
                formatter,
                "a document with confidential fields must be written outside its source tree, which is {}",
                root.display()
            ),
            Self::OutputPath { path } => {
                write!(
                    formatter,
                    "the output path {} has no file name",
                    path.display()
                )
            }
            Self::CreateDirectory { path, source } => {
                write!(formatter, "cannot create {}: {source}", path.display())
            }
            Self::Write { path, source } => {
                write!(formatter, "cannot write {}: {source}", path.display())
            }
            Self::Replace { path, source } => {
                write!(formatter, "cannot replace {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. }
            | Self::CreateDirectory { source, .. }
            | Self::Write { source, .. }
            | Self::Replace { source, .. } => Some(source),
            Self::Markdown(error) => Some(error),
            Self::Confidential(error) => Some(error),
            Self::Template(error) => Some(error),
            Self::Pdf(error) => Some(error),
            Self::Odt(error) => Some(error),
            Self::Docx(error) => Some(error),
            Self::ConfidentialOutput { .. } | Self::OutputPath { .. } => None,
        }
    }
}

impl From<MarkdownError> for ExportError {
    fn from(error: MarkdownError) -> Self {
        Self::Markdown(error)
    }
}

impl From<ResolveError> for ExportError {
    fn from(error: ResolveError) -> Self {
        Self::Confidential(error)
    }
}

impl From<LoadError> for ExportError {
    fn from(error: LoadError) -> Self {
        Self::Template(error)
    }
}

impl From<PdfError> for ExportError {
    fn from(error: PdfError) -> Self {
        Self::Pdf(error)
    }
}

impl From<OdtError> for ExportError {
    fn from(error: OdtError) -> Self {
        Self::Odt(error)
    }
}

impl From<DocxError> for ExportError {
    fn from(error: DocxError) -> Self {
        Self::Docx(error)
    }
}

/// Reads, parses and validates a document and its template without writing.
pub fn check(job: &DocumentJob, store: &ConfidentialStore) -> Result<CheckOutcome, ExportError> {
    let text = job.source.text()?;
    let document = markdown::parse(&text)?;
    let resolution = confidential::resolve(&document, store)?;
    let resolved = resolve_template(job)?;
    Ok(CheckOutcome {
        blocks: document.block_count(),
        headings: document.heading_count(),
        template: resolved.origin().to_string(),
        confidential: resolution
            .names()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
    })
}

/// Reads, parses, resolves and renders a document, without writing.
pub fn render(
    job: &DocumentJob,
    format: Format,
    store: &ConfidentialStore,
) -> Result<RenderOutcome, ExportError> {
    let text = job.source.text()?;
    let document = markdown::parse(&text)?;
    let resolution = confidential::resolve(&document, store)?;
    let resolved = resolve_template(job)?;
    let rendered = render_document(job, format, resolution.document(), &resolved)?;
    Ok(RenderOutcome {
        bytes: rendered.bytes().to_vec(),
        warnings: rendered.warnings().to_vec(),
    })
}

/// Reads, parses, resolves and renders a document, then writes the output.
pub fn export(
    job: &DocumentJob,
    format: Format,
    output: &Path,
    store: &ConfidentialStore,
) -> Result<ExportOutcome, ExportError> {
    let text = job.source.text()?;
    let document = markdown::parse(&text)?;
    let resolution = confidential::resolve(&document, store)?;
    let resolved = resolve_template(job)?;
    if !resolution.names().is_empty() {
        require_confidential_output(job, output)?;
    }
    let rendered = render_document(job, format, resolution.document(), &resolved)?;
    write_atomic(output, rendered.bytes())?;
    Ok(ExportOutcome {
        output: output.to_path_buf(),
        warnings: rendered.warnings().to_vec(),
    })
}

fn render_document(
    job: &DocumentJob,
    format: Format,
    document: &Document,
    resolved: &ResolvedTemplate,
) -> Result<Rendered, ExportError> {
    let template = resolved.template();
    let source_directory = job
        .source_path()
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let rendered = match format {
        Format::Pdf => {
            let assets = pdf::load_assets(
                document,
                template,
                source_directory,
                resolved.base_directory(),
            )?;
            PdfRenderer::new(assets).render(document, template)?
        }
        Format::Txt => render_infallible(TxtRenderer.render(document, template)),
        Format::Odt => OdtRenderer::load(
            document,
            template,
            source_directory,
            resolved.base_directory(),
        )?
        .render(document, template)?,
        Format::Docx => DocxRenderer::load(
            document,
            template,
            source_directory,
            resolved.base_directory(),
        )?
        .render(document, template)?,
    };
    Ok(rendered)
}

/// Unwraps a renderer whose error type cannot be constructed.
fn render_infallible(result: Result<Rendered, std::convert::Infallible>) -> Rendered {
    match result {
        Ok(rendered) => rendered,
        Err(never) => match never {},
    }
}

/// Resolves the template for a source, without parsing the document.
pub fn resolve_template(job: &DocumentJob) -> Result<ResolvedTemplate, ExportError> {
    match job.template.as_deref() {
        Some(path) => {
            let template = loader::load_file(path)?;
            Ok(ResolvedTemplate::new(
                TemplateOrigin::Explicit(path.to_owned()),
                template,
            ))
        }
        None => {
            let directory = job
                .source_path()
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            Ok(loader::resolve(directory, job.global_template.as_deref())?)
        }
    }
}

/// A document with confidential fields must be written outside its source tree.
fn require_confidential_output(job: &DocumentJob, output: &Path) -> Result<(), ExportError> {
    let directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if confidential::is_within(job.source_root(), directory) {
        return Err(ExportError::ConfidentialOutput {
            root: job.source_root().to_path_buf(),
        });
    }
    Ok(())
}

/// Writes a file through a temporary file in the same directory.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ExportError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| ExportError::CreateDirectory {
            path: parent.to_owned(),
            source,
        })?;
    }
    let temporary = temporary_path(path)?;
    if let Err(source) = fs::write(&temporary, bytes) {
        let _ = fs::remove_file(&temporary);
        return Err(ExportError::Write {
            path: temporary,
            source,
        });
    }
    if let Err(source) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ExportError::Replace {
            path: path.to_owned(),
            source,
        });
    }
    Ok(())
}

fn temporary_path(path: &Path) -> Result<PathBuf, ExportError> {
    let mut name = path
        .file_name()
        .ok_or_else(|| ExportError::OutputPath {
            path: path.to_owned(),
        })?
        .to_os_string();
    name.push(".tmp");
    Ok(path.with_file_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn an_atomic_write_creates_the_parent_and_removes_the_temporary_file() {
        let project = TempDir::new().expect("a temporary project");
        let path = project.path().join("out/A.pdf");

        write_atomic(&path, b"new").expect("the file must be written");

        assert_eq!(fs::read(&path).expect("the file must exist"), b"new");
        assert!(!path.with_file_name("A.pdf.tmp").exists());
    }

    #[test]
    fn a_buffer_is_checked_without_the_source_file() {
        let project = TempDir::new().expect("a temporary project");
        let job = DocumentJob::new(
            Source::Buffer {
                text: "# Título\n\nTexto.\n".to_owned(),
                path: project.path().join("A.md"),
            },
            project.path().to_path_buf(),
        );

        let outcome = check(&job, &ConfidentialStore::default()).expect("the document must check");

        assert_eq!(outcome.blocks(), 2);
        assert_eq!(outcome.headings(), 1);
        assert_eq!(outcome.template(), "embedded");
        assert!(outcome.confidential().is_empty());
    }

    #[test]
    fn a_buffer_is_exported_without_the_source_file() {
        let project = TempDir::new().expect("a temporary project");
        let output = project.path().join("out/A.txt");
        let job = DocumentJob::new(
            Source::Buffer {
                text: "# Título\n\nTexto.\n".to_owned(),
                path: project.path().join("A.md"),
            },
            project.path().to_path_buf(),
        );

        let outcome = export(&job, Format::Txt, &output, &ConfidentialStore::default())
            .expect("the document must export");

        assert_eq!(outcome.output(), output);
        let text = fs::read_to_string(&output).expect("the output must exist");
        assert!(text.contains("Título"), "{text}");
        assert!(text.contains("Texto."), "{text}");
    }

    #[test]
    fn a_buffer_is_rendered_without_writing() {
        let project = TempDir::new().expect("a temporary project");
        let job = DocumentJob::new(
            Source::Buffer {
                text: "# Título\n\nTexto.\n".to_owned(),
                path: project.path().join("A.md"),
            },
            project.path().to_path_buf(),
        );

        let outcome = render(&job, Format::Txt, &ConfidentialStore::default())
            .expect("the document must render");

        let text = std::str::from_utf8(outcome.bytes()).expect("the text must be UTF-8");
        assert!(text.contains("Título"), "{text}");
        assert!(text.contains("Texto."), "{text}");
        assert!(outcome.warnings().is_empty());
    }

    #[test]
    fn a_buffer_with_confidential_fields_needs_an_output_outside_the_source_tree() {
        let project = TempDir::new().expect("a temporary project");
        let job = DocumentJob::new(
            Source::Buffer {
                text: "Texto {NOME}.\n".to_owned(),
                path: project.path().join("A.md"),
            },
            project.path().to_path_buf(),
        );

        let inside = project.path().join("A.pdf");
        let error = require_confidential_output(&job, &inside).expect_err("inside the tree");
        assert!(
            matches!(error, ExportError::ConfidentialOutput { .. }),
            "{error}"
        );

        let outside = TempDir::new().expect("another project");
        require_confidential_output(&job, &outside.path().join("A.pdf")).expect("outside the tree");
    }

    #[test]
    fn a_missing_source_file_is_an_error() {
        let project = TempDir::new().expect("a temporary project");
        let job = DocumentJob::new(
            Source::File(project.path().join("missing.md")),
            project.path().to_path_buf(),
        );

        let error = check(&job, &ConfidentialStore::default()).expect_err("a missing file");

        assert!(matches!(error, ExportError::Read { .. }), "{error}");
        assert!(error.to_string().contains("cannot read"), "{error}");
    }
}
