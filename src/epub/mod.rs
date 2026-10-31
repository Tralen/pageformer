//! EPUB backend: an EPUB 3 package built from the HTML content.
//!
//! Chapters split at first-level headings, the CSS is a reflowable subset of
//! the template and the ZIP is written by the internal ZIP writer with a fixed timestamp,
//! so two runs with the same input produce identical bytes.

mod chapters;
mod package;

use std::fmt;
use std::path::Path;

use crate::assets::{self, AssetError, Assets};
use crate::document::Document;
use crate::render::{Rendered, Renderer};
use crate::template::Template;
use crate::zip::ZipError;

/// The media type stored as the first, uncompressed entry.
pub const MIMETYPE: &str = "application/epub+zip";

/// The EPUB backend with its resolved body images.
#[derive(Debug, Clone)]
pub struct EpubRenderer {
    assets: Assets,
}

impl EpubRenderer {
    /// Loads the body images the document references.
    ///
    /// The letterhead is intentionally not loaded: EPUB is reflowable and does
    /// not use the page background.
    pub fn load(document: &Document, source_directory: &Path) -> Result<Self, EpubError> {
        let assets = assets::load_images(document, source_directory)?;
        Ok(Self { assets })
    }
}

impl Renderer for EpubRenderer {
    type Error = EpubError;

    fn render(&self, document: &Document, template: &Template) -> Result<Rendered, Self::Error> {
        render(document, template, &self.assets)
    }
}

/// Renders a document with a template and resolved body images into an EPUB.
pub fn render(
    document: &Document,
    template: &Template,
    assets: &Assets,
) -> Result<Rendered, EpubError> {
    let built = package::build(document, template, assets)?;
    Ok(Rendered::new(built.bytes, built.warnings))
}

/// Why an EPUB package could not be produced.
#[derive(Debug)]
pub enum EpubError {
    Asset(AssetError),
    Package(ZipError),
    MissingImage { name: String },
    UnsupportedImage { name: String },
}

impl fmt::Display for EpubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(formatter, "{error}"),
            Self::Package(error) => write!(formatter, "cannot build the EPUB package: {error}"),
            Self::MissingImage { name } => write!(formatter, "image {name} was not loaded"),
            Self::UnsupportedImage { name } => {
                write!(formatter, "unsupported media type for {name}")
            }
        }
    }
}

impl std::error::Error for EpubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::Package(error) => Some(error),
            Self::MissingImage { .. } | Self::UnsupportedImage { .. } => None,
        }
    }
}

impl From<AssetError> for EpubError {
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mimetype_is_the_epub_media_type() {
        assert_eq!(MIMETYPE, "application/epub+zip");
    }
}
