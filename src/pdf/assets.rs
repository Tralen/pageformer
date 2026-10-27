//! Loads the files a PDF needs into the Typst project.
//!
//! The generator never touches the file system. Every local image is read here,
//! stored under a synthetic project path and exposed to the Typst world by
//! [`Assets::files`].

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use typst::foundations::Bytes;

use crate::document::{Block, Document, Inline};
use crate::template::Template;

use super::PdfError;

/// The virtual project paths assigned to the files used by a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetMap {
    letterhead: Option<String>,
    images: BTreeMap<String, String>,
}

impl AssetMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_letterhead(mut self, asset: impl Into<String>) -> Self {
        self.letterhead = Some(asset.into());
        self
    }

    pub fn with_image(mut self, destination: impl Into<String>, asset: impl Into<String>) -> Self {
        self.images.insert(destination.into(), asset.into());
        self
    }

    pub fn letterhead(&self) -> Option<&str> {
        self.letterhead.as_deref()
    }

    pub fn image(&self, destination: &str) -> Option<&str> {
        self.images.get(destination).map(String::as_str)
    }
}

/// The resolved assets and their bytes.
#[derive(Debug, Clone, Default)]
pub struct Assets {
    map: AssetMap,
    files: BTreeMap<String, Bytes>,
}

impl Assets {
    pub fn map(&self) -> &AssetMap {
        &self.map
    }

    pub(crate) fn files(&self) -> &BTreeMap<String, Bytes> {
        &self.files
    }
}

/// Reads the letterhead and the images of a document.
///
/// Image destinations resolve relative to the Markdown file and the letterhead
/// resolves relative to the template file. Remote destinations are rejected
/// because the binary never performs network requests.
pub fn load_assets(
    document: &Document,
    template: &Template,
    source_directory: &Path,
    template_directory: Option<&Path>,
) -> Result<Assets, PdfError> {
    let mut map = AssetMap::new();
    let mut files = BTreeMap::new();
    let mut destinations = Vec::new();
    collect_block_images(document.blocks(), &mut destinations);

    let mut seen = BTreeSet::new();
    let mut index = 0_usize;
    for destination in destinations {
        if !seen.insert(destination) {
            continue;
        }
        let path = resolve_image(destination, source_directory)?;
        let extension = supported_extension(&path)?;
        let asset = format!("assets/image-{index}.{extension}");
        index += 1;
        files.insert(asset.clone(), read_asset(&path)?);
        map = map.with_image(destination, asset);
    }

    if let Some(letterhead) = template.page().letterhead() {
        let directory = template_directory.ok_or(PdfError::MissingTemplateDirectory)?;
        let path = directory.join(letterhead.as_str());
        let extension = supported_extension(&path)?;
        let asset = format!("assets/letterhead.{extension}");
        files.insert(asset.clone(), read_asset(&path)?);
        map = map.with_letterhead(asset);
    }

    Ok(Assets { map, files })
}

fn collect_block_images<'a>(blocks: &'a [Block], destinations: &mut Vec<&'a str>) {
    for block in blocks {
        match block {
            Block::Heading(heading) => collect_inline_images(heading.content(), destinations),
            Block::Paragraph(inlines) => collect_inline_images(inlines, destinations),
            Block::NumberedParagraph(paragraph) => {
                collect_inline_images(paragraph.content(), destinations);
            }
            Block::List(list) => {
                for item in list.items() {
                    collect_block_images(item.blocks(), destinations);
                }
            }
            Block::Table(table) => {
                for cell in table.header() {
                    collect_inline_images(cell.content(), destinations);
                }
                for row in table.rows() {
                    for cell in row {
                        collect_inline_images(cell.content(), destinations);
                    }
                }
            }
            Block::Quote(quote) => collect_block_images(quote.blocks(), destinations),
            Block::FootnoteDefinition(definition) => {
                collect_block_images(definition.blocks(), destinations);
            }
            Block::CodeBlock(_) | Block::ThematicBreak | Block::PageBreak => {}
        }
    }
}

fn collect_inline_images<'a>(inlines: &'a [Inline], destinations: &mut Vec<&'a str>) {
    for inline in inlines {
        match inline {
            Inline::Image { destination, .. } => destinations.push(destination.as_str()),
            Inline::Emphasis(content)
            | Inline::Strong(content)
            | Inline::Strikethrough(content) => collect_inline_images(content, destinations),
            Inline::Link { content, .. } => collect_inline_images(content, destinations),
            Inline::Text(_)
            | Inline::Code(_)
            | Inline::SoftBreak
            | Inline::HardBreak
            | Inline::FootnoteReference(_) => {}
        }
    }
}

fn resolve_image(destination: &str, source_directory: &Path) -> Result<PathBuf, PdfError> {
    let path = Path::new(destination);
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    if destination.is_empty() || destination.contains(':') {
        return Err(PdfError::UnsupportedImage {
            destination: destination.to_owned(),
        });
    }
    Ok(source_directory.join(path))
}

fn supported_extension(path: &Path) -> Result<String, PdfError> {
    let Some(extension) = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
    else {
        return Err(PdfError::UnsupportedImageFormat {
            path: path.to_owned(),
        });
    };
    if matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp"
    ) {
        Ok(extension)
    } else {
        Err(PdfError::UnsupportedImageFormat {
            path: path.to_owned(),
        })
    }
}

fn read_asset(path: &Path) -> Result<Bytes, PdfError> {
    fs::read(path)
        .map(Bytes::new)
        .map_err(|source| PdfError::AssetRead {
            path: path.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Destination, Metadata};
    use crate::template::Template;
    use tempfile::TempDir;

    fn document_with_image(destination: &str) -> Document {
        Document::new(
            Metadata::default(),
            vec![Block::Paragraph(vec![Inline::Image {
                destination: Destination::new(destination),
                title: None,
                alternative: vec![Inline::Text("Logo".to_owned())],
            }])],
        )
    }

    fn embedded_template() -> Template {
        Template::embedded().expect("the embedded template must be valid")
    }

    #[test]
    fn local_images_are_read_and_deduplicated() {
        let project = TempDir::new().expect("a temporary project");
        fs::write(project.path().join("logo.png"), b"not a real png")
            .expect("the image must be written");
        let document = Document::new(
            Metadata::default(),
            vec![
                Block::Paragraph(vec![Inline::Image {
                    destination: Destination::new("logo.png"),
                    title: None,
                    alternative: Vec::new(),
                }]),
                Block::Paragraph(vec![Inline::Image {
                    destination: Destination::new("logo.png"),
                    title: None,
                    alternative: Vec::new(),
                }]),
            ],
        );

        let assets = load_assets(&document, &embedded_template(), project.path(), None)
            .expect("the assets must load");
        assert_eq!(assets.map().image("logo.png"), Some("assets/image-0.png"));
        assert_eq!(assets.files.len(), 1);
    }

    #[test]
    fn a_missing_image_is_an_error() {
        let project = TempDir::new().expect("a temporary project");
        let document = document_with_image("missing.png");
        let error = load_assets(&document, &embedded_template(), project.path(), None)
            .expect_err("a missing image");
        assert!(matches!(error, PdfError::AssetRead { .. }), "{error}");
    }

    #[test]
    fn remote_images_are_rejected() {
        let project = TempDir::new().expect("a temporary project");
        let document = document_with_image("https://example.org/logo.png");
        let error = load_assets(&document, &embedded_template(), project.path(), None)
            .expect_err("a remote image");
        assert!(
            matches!(error, PdfError::UnsupportedImage { .. }),
            "{error}"
        );
    }

    #[test]
    fn images_without_a_supported_extension_are_rejected() {
        let project = TempDir::new().expect("a temporary project");
        fs::write(project.path().join("logo.bmp"), b"bitmap").expect("the image must be written");
        let document = document_with_image("logo.bmp");
        let error = load_assets(&document, &embedded_template(), project.path(), None)
            .expect_err("an unsupported image");
        assert!(
            matches!(error, PdfError::UnsupportedImageFormat { .. }),
            "{error}"
        );
    }

    #[test]
    fn the_letterhead_resolves_against_the_template_directory() {
        let project = TempDir::new().expect("a temporary project");
        let templates = project.path().join("templates");
        fs::create_dir_all(&templates).expect("the template directory must be created");
        fs::write(templates.join("letterhead.png"), b"letterhead")
            .expect("the letterhead must be written");
        let template =
            Template::from_yaml("version: \"0.5\"\npage:\n  letterhead: letterhead.png\n")
                .expect("the template must be valid");
        let document = Document::new(Metadata::default(), Vec::new());

        let assets = load_assets(&document, &template, project.path(), Some(&templates))
            .expect("the assets must load");
        assert_eq!(assets.map().letterhead(), Some("assets/letterhead.png"));
    }

    #[test]
    fn a_letterhead_without_a_template_directory_is_an_error() {
        let project = TempDir::new().expect("a temporary project");
        let template =
            Template::from_yaml("version: \"0.5\"\npage:\n  letterhead: letterhead.png\n")
                .expect("the template must be valid");
        let document = Document::new(Metadata::default(), Vec::new());
        let error = load_assets(&document, &template, project.path(), None)
            .expect_err("a letterhead without a base directory");
        assert!(matches!(error, PdfError::MissingTemplateDirectory));
    }
}
