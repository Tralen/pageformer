//! DOCX backend: an Office Open XML package with the template's styles.
//!
//! The package is a stored ZIP written by [`crate::zip`], with fixed
//! relationship ids, numbering ids and property dates, so two runs with the
//! same input produce identical bytes.

mod content;
mod numbering;
mod package;
mod styles;

use std::fmt;
use std::path::Path;

use crate::assets::{self, AssetError, Assets};
use crate::document::Document;
use crate::render::{Rendered, Renderer};
use crate::template::Template;
use crate::zip::{self, Entry, ZipError};

/// The DOCX backend with its resolved assets.
#[derive(Debug, Clone)]
pub struct DocxRenderer {
    assets: Assets,
}

impl DocxRenderer {
    /// Loads the letterhead and the images the document references.
    pub fn load(
        document: &Document,
        template: &Template,
        source_directory: &Path,
        template_directory: Option<&Path>,
    ) -> Result<Self, DocxError> {
        let assets = assets::load_assets(document, template, source_directory, template_directory)?;
        Ok(Self { assets })
    }
}

impl Renderer for DocxRenderer {
    type Error = DocxError;

    fn render(&self, document: &Document, template: &Template) -> Result<Rendered, Self::Error> {
        Ok(Rendered::new(
            render(document, template, &self.assets)?,
            Vec::new(),
        ))
    }
}

/// Renders a document with a template into a DOCX package.
pub fn render(
    document: &Document,
    template: &Template,
    assets: &Assets,
) -> Result<Vec<u8>, DocxError> {
    let media = content::media_plan(assets);
    let built = content::build(document, template, assets, &media);
    let parts = package::RunningParts::new(built.header.as_deref(), built.footer.as_deref());
    let content_types = package::content_types(&media, parts)?;
    let relationships = package::document_relationships(&media, &built.hyperlinks, parts);
    let styles = styles::styles_xml(document, template);
    let numbering = built.numbering.xml(template);
    let core = package::core_xml(document);
    let app = package::app_xml();
    let settings = package::settings_xml(template.page().background());
    let root = package::root_relationships();

    let mut entries = vec![
        Entry::new("[Content_Types].xml", content_types.as_bytes()),
        Entry::new("_rels/.rels", root.as_bytes()),
        Entry::new("docProps/core.xml", core.as_bytes()),
        Entry::new("docProps/app.xml", app.as_bytes()),
        Entry::new("word/document.xml", built.document.as_bytes()),
        Entry::new("word/_rels/document.xml.rels", relationships.as_bytes()),
        Entry::new("word/styles.xml", styles.as_bytes()),
        Entry::new("word/numbering.xml", numbering.as_bytes()),
        Entry::new("word/footnotes.xml", built.footnotes.as_bytes()),
        Entry::new("word/settings.xml", settings.as_bytes()),
    ];
    let header_relationships = package::header_relationships(&media, assets.map().letterhead());
    if let Some(header) = &built.header {
        entries.push(Entry::new("word/header1.xml", header.as_bytes()));
        if let Some(relationships) = &header_relationships {
            entries.push(Entry::new(
                "word/_rels/header1.xml.rels",
                relationships.as_bytes(),
            ));
        }
    }
    if let Some(footer) = &built.footer {
        entries.push(Entry::new("word/footer1.xml", footer.as_bytes()));
    }
    for entry in &media {
        let bytes =
            assets
                .files()
                .get(&entry.virtual_name)
                .ok_or_else(|| DocxError::MissingAsset {
                    name: entry.virtual_name.clone(),
                })?;
        entries.push(Entry::new(entry.package_path.as_str(), bytes));
    }
    zip::write_stored(&entries).map_err(DocxError::Package)
}

/// Why a DOCX package could not be produced.
#[derive(Debug)]
pub enum DocxError {
    Asset(AssetError),
    Package(ZipError),
    UnsupportedMediaType { name: String },
    MissingAsset { name: String },
}

impl fmt::Display for DocxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(formatter, "{error}"),
            Self::Package(error) => write!(formatter, "cannot build the DOCX package: {error}"),
            Self::UnsupportedMediaType { name } => {
                write!(formatter, "unsupported media type for {name}")
            }
            Self::MissingAsset { name } => {
                write!(formatter, "the packaged image {name} is missing")
            }
        }
    }
}

impl std::error::Error for DocxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::Package(error) => Some(error),
            Self::UnsupportedMediaType { .. } | Self::MissingAsset { .. } => None,
        }
    }
}

impl From<AssetError> for DocxError {
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}
