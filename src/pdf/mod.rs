//! PDF backend built on Typst.
//!
//! The document model becomes Typst source, which Typst compiles with a small
//! world that serves embedded fonts, system fonts and the images read by the
//! asset loader. The output is PDF/A-2b and the date is fixed, so two runs over
//! the same input produce the same bytes.

mod assets;
mod source;
mod world;

use std::fmt;
use std::io;
use std::path::PathBuf;

use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards};

use crate::document::Document;
use crate::template::Template;

use self::world::{PdfWorld, format_diagnostics};

pub use assets::{AssetMap, Assets, load_assets};
pub use source::typst_source;

pub(crate) const FIXED_YEAR: i32 = 2026;
pub(crate) const FIXED_MONTH: u8 = 1;
pub(crate) const FIXED_DAY: u8 = 1;

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

/// Renders a document with a template into PDF/A-2b.
pub fn render(
    document: &Document,
    template: &Template,
    assets: &Assets,
) -> Result<PdfOutput, PdfError> {
    let source = typst_source(document, template, assets.map())?;
    let world = PdfWorld::new(source, assets.files())?;
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
            | Self::MissingTemplateDirectory => None,
        }
    }
}
