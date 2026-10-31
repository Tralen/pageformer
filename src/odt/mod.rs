//! ODT backend: an OpenDocument text package with the template's semantic styles.
//!
//! The package is a stored ZIP written by `crate::zip`, so two runs with the
//! same input produce identical bytes. The parts are `mimetype` first, then the
//! manifest, `content.xml`, `styles.xml`, `meta.xml` and the pictures.

mod content;
mod meta;
mod styles;

use std::fmt;

use crate::assets::{self, AssetError, Assets};
use crate::document::Document;
use crate::render::{Rendered, Renderer};
use crate::template::Template;
use crate::zip::{self, Entry, ZipError};

/// The media type stored as the first, uncompressed entry.
pub const MIMETYPE: &str = "application/vnd.oasis.opendocument.text";

/// The ODT backend with its resolved assets.
#[derive(Debug, Clone)]
pub struct OdtRenderer {
    assets: Assets,
}

impl OdtRenderer {
    /// Loads the letterhead and the images the document references.
    pub fn load(
        document: &Document,
        template: &Template,
        source_directory: &std::path::Path,
        template_directory: Option<&std::path::Path>,
    ) -> Result<Self, OdtError> {
        let assets = assets::load_assets(document, template, source_directory, template_directory)?;
        Ok(Self { assets })
    }
}

impl Renderer for OdtRenderer {
    type Error = OdtError;

    fn render(&self, document: &Document, template: &Template) -> Result<Rendered, Self::Error> {
        Ok(Rendered::new(
            render(document, template, &self.assets)?,
            Vec::new(),
        ))
    }
}

/// Renders a document with a template into an ODT package.
pub fn render(
    document: &Document,
    template: &Template,
    assets: &Assets,
) -> Result<Vec<u8>, OdtError> {
    let content = content::content_xml(document, template, assets);
    let styles = styles::styles_xml(document, template, assets);
    let metadata = meta::meta_xml(document);
    let manifest = meta::manifest_xml(assets)?;
    let pictures = pictures(assets)?;

    let mut entries = vec![
        Entry::new("mimetype", MIMETYPE.as_bytes()),
        Entry::new("META-INF/manifest.xml", manifest.as_bytes()),
        Entry::new("content.xml", content.as_bytes()),
        Entry::new("styles.xml", styles.as_bytes()),
        Entry::new("meta.xml", metadata.as_bytes()),
    ];
    for (name, bytes) in &pictures {
        entries.push(Entry::new(name.as_str(), bytes));
    }
    zip::write_stored(&entries).map_err(OdtError::Package)
}

fn pictures(assets: &Assets) -> Result<Vec<(String, &[u8])>, OdtError> {
    let mut pictures = Vec::new();
    for (virtual_name, bytes) in assets.files() {
        let name = picture_name(virtual_name);
        if media_type(name).is_none() {
            return Err(OdtError::UnsupportedMediaType {
                name: name.to_owned(),
            });
        }
        pictures.push((format!("Pictures/{name}"), bytes.as_slice()));
    }
    Ok(pictures)
}

pub(crate) fn picture_name(virtual_name: &str) -> &str {
    match virtual_name.rsplit_once('/') {
        Some((_, name)) => name,
        None => virtual_name,
    }
}

pub(crate) use crate::assets::media_type;

/// Why an ODT package could not be produced.
#[derive(Debug)]
pub enum OdtError {
    Asset(AssetError),
    Package(ZipError),
    UnsupportedMediaType { name: String },
}

impl fmt::Display for OdtError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(formatter, "{error}"),
            Self::Package(error) => write!(formatter, "cannot build the ODT package: {error}"),
            Self::UnsupportedMediaType { name } => {
                write!(formatter, "unsupported media type for {name}")
            }
        }
    }
}

impl std::error::Error for OdtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::Package(error) => Some(error),
            Self::UnsupportedMediaType { .. } => None,
        }
    }
}

impl From<AssetError> for OdtError {
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_virtual_name_keeps_only_its_file_name() {
        assert_eq!(picture_name("assets/image-0.png"), "image-0.png");
        assert_eq!(picture_name("letterhead.png"), "letterhead.png");
    }

    #[test]
    fn known_extensions_have_a_media_type() {
        assert_eq!(media_type("a.png"), Some("image/png"));
        assert_eq!(media_type("a.JPG"), Some("image/jpeg"));
        assert_eq!(media_type("a.svg"), Some("image/svg+xml"));
        assert_eq!(media_type("a.bmp"), None);
    }
}
