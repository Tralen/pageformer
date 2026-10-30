//! PDF backend built on Typst.
//!
//! The document model becomes Typst source, which Typst compiles with a small
//! world that serves embedded fonts, system fonts and the images read by the
//! asset loader. The output is PDF/A-2b and the date is fixed, so two runs over
//! the same input produce the same bytes.

mod source;
mod world;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards};

use crate::assets::{self, AssetError};
use crate::document::{Document, DocumentDate};
use crate::render::{Rendered, Renderer};
use crate::template::Template;

use self::world::{PdfWorld, format_diagnostics};

pub use crate::assets::{AssetMap, Assets};
pub use source::typst_source;

/// Reads the letterhead and the images of a document.
pub fn load_assets(
    document: &Document,
    template: &Template,
    source_directory: &Path,
    template_directory: Option<&Path>,
) -> Result<Assets, PdfError> {
    assets::load_assets(document, template, source_directory, template_directory)
        .map_err(PdfError::from)
}

impl From<AssetError> for PdfError {
    fn from(error: AssetError) -> Self {
        match error {
            AssetError::UnsupportedImage { destination } => Self::UnsupportedImage { destination },
            AssetError::AssetRead { path, source } => Self::AssetRead { path, source },
            AssetError::UnsupportedImageFormat { path } => Self::UnsupportedImageFormat { path },
            AssetError::MissingTemplateDirectory => Self::MissingTemplateDirectory,
        }
    }
}

pub(crate) const FIXED_YEAR: i32 = 2026;
pub(crate) const FIXED_MONTH: u8 = 1;
pub(crate) const FIXED_DAY: u8 = 1;

/// The date used when the frontmatter declares none, so the output stays
/// deterministic.
const FIXED_DATE: DocumentDate = DocumentDate::from_parts(FIXED_YEAR, FIXED_MONTH, FIXED_DAY);

/// A rendered PDF and the warnings Typst reported.
#[derive(Debug, Clone)]
pub struct PdfOutput {
    bytes: Vec<u8>,
    warnings: Vec<String>,
}

impl PdfOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// The PDF backend with its resolved assets.
#[derive(Debug, Clone)]
pub struct PdfRenderer {
    assets: Assets,
}

impl PdfRenderer {
    /// Wraps the assets loaded by [`load_assets`].
    pub fn new(assets: Assets) -> Self {
        Self { assets }
    }
}

impl Renderer for PdfRenderer {
    type Error = PdfError;

    fn render(&self, document: &Document, template: &Template) -> Result<Rendered, Self::Error> {
        let output = crate::pdf::render(document, template, &self.assets)?;
        let warnings = output.warnings().to_vec();
        Ok(Rendered::new(output.into_bytes(), warnings))
    }
}

/// Renders a document with a template into PDF/A-2b.
pub fn render(
    document: &Document,
    template: &Template,
    assets: &Assets,
) -> Result<PdfOutput, PdfError> {
    let source = typst_source(document, template, assets.map())?;
    let date = document.metadata().date().unwrap_or(FIXED_DATE);
    let world = PdfWorld::new(source, assets.files(), date)?;
    let warned = typst::compile::<PagedDocument>(&world);
    let warnings = warned
        .warnings
        .iter()
        .map(|warning| world::format_diagnostic(&world, warning))
        .collect();
    let compiled = warned
        .output
        .map_err(|errors| PdfError::Compilation(format_diagnostics(&world, &errors)))?;
    let standards = PdfStandards::new(&[PdfStandard::A_2b])
        .map_err(|error| PdfError::Standards(error.message().to_string()))?;
    let options = PdfOptions {
        standards,
        ..PdfOptions::default()
    };
    let bytes = typst_pdf::pdf(&compiled, &options)
        .map_err(|errors| PdfError::Export(format_diagnostics(&world, &errors)))?;
    Ok(PdfOutput { bytes, warnings })
}

/// Why a PDF could not be generated.
#[derive(Debug)]
pub enum PdfError {
    InvalidPath,
    Compilation(String),
    Standards(String),
    Export(String),
    MissingAsset { destination: String },
    UnsupportedImage { destination: String },
    AssetRead { path: PathBuf, source: io::Error },
    UnsupportedImageFormat { path: PathBuf },
    MissingTemplateDirectory,
    InvalidDate,
}

impl fmt::Display for PdfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => formatter.write_str("invalid virtual path"),
            Self::Compilation(message) => write!(formatter, "compilation failed: {message}"),
            Self::Standards(message) => write!(formatter, "invalid PDF standard: {message}"),
            Self::Export(message) => write!(formatter, "PDF export failed: {message}"),
            Self::MissingAsset { destination } => {
                write!(formatter, "image `{destination}` has no loaded asset")
            }
            Self::UnsupportedImage { destination } => write!(
                formatter,
                "image `{destination}` is not a local file; remote images are not supported"
            ),
            Self::AssetRead { path, source } => {
                write!(formatter, "cannot read image {}: {source}", path.display())
            }
            Self::UnsupportedImageFormat { path } => write!(
                formatter,
                "unsupported image format for {}; use png, jpg, gif, svg or webp",
                path.display()
            ),
            Self::MissingTemplateDirectory => formatter
                .write_str("the template has a letterhead but no directory to resolve it from"),
            Self::InvalidDate => formatter.write_str("the document date is not a calendar date"),
        }
    }
}

impl std::error::Error for PdfError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AssetRead { source, .. } => Some(source),
            Self::InvalidPath
            | Self::Compilation(_)
            | Self::Standards(_)
            | Self::Export(_)
            | Self::MissingAsset { .. }
            | Self::UnsupportedImage { .. }
            | Self::UnsupportedImageFormat { .. }
            | Self::MissingTemplateDirectory
            | Self::InvalidDate => None,
        }
    }
}
