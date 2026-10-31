//! HTML backend: one self-contained, semantic file.
//!
//! The output embeds the stylesheet and the images as data URIs, uses no
//! network resource, and derives the print rules from the template. The body is
//! XHTML-compatible HTML5, so the structural tests can parse it without an HTML
//! parser and the EPUB backend can reuse the same block conversion.

pub(crate) mod content;
pub(crate) mod css;
mod data_uri;
mod escape;

use std::fmt;
use std::path::Path;

use crate::assets::{self, AssetError, Assets};
use crate::document::Document;
use crate::render::{Rendered, Renderer};
use crate::template::Template;

/// The HTML backend with its resolved assets.
#[derive(Debug, Clone)]
pub struct HtmlRenderer {
    assets: Assets,
}

impl HtmlRenderer {
    /// Loads the letterhead and the images the document references.
    pub fn load(
        document: &Document,
        template: &Template,
        source_directory: &Path,
        template_directory: Option<&Path>,
    ) -> Result<Self, HtmlError> {
        let assets = assets::load_assets(document, template, source_directory, template_directory)?;
        Ok(Self { assets })
    }
}

impl Renderer for HtmlRenderer {
    type Error = HtmlError;

    fn render(&self, document: &Document, template: &Template) -> Result<Rendered, Self::Error> {
        Ok(render(document, template, &self.assets))
    }
}

/// Renders a document with a template into a self-contained HTML file.
pub fn render(document: &Document, template: &Template, assets: &Assets) -> Rendered {
    let content = content::render(document, template, assets);
    let main = content.full();
    let stylesheet = css::document_stylesheet(template, document.metadata(), assets);
    let metadata = document.metadata();
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n<html");
    if let Some(language) = metadata.language() {
        html.push_str(&format!(
            " lang=\"{}\"",
            escape::attribute(language.as_str())
        ));
    }
    html.push_str(">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\" />\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n");
    html.push_str(&format!(
        "<title>{}</title>\n",
        metadata
            .title()
            .map_or_else(String::new, |title| escape::text(title.as_str()))
    ));
    if let Some(author) = metadata.author() {
        html.push_str(&format!(
            "<meta name=\"author\" content=\"{}\" />\n",
            escape::attribute(author.as_str())
        ));
    }
    if let Some(subject) = metadata.subject() {
        html.push_str(&format!(
            "<meta name=\"description\" content=\"{}\" />\n",
            escape::attribute(subject.as_str())
        ));
    }
    if !metadata.keywords().is_empty() {
        let keywords = metadata
            .keywords()
            .iter()
            .map(|keyword| keyword.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        html.push_str(&format!(
            "<meta name=\"keywords\" content=\"{}\" />\n",
            escape::attribute(&keywords)
        ));
    }
    if let Some(date) = metadata.date() {
        html.push_str(&format!(
            "<meta name=\"date\" content=\"{}\" />\n",
            escape::attribute(&date.to_string())
        ));
    }
    html.push_str("<style>\n/*<![CDATA[*/\n");
    html.push_str(&stylesheet);
    html.push_str("/*]]>*/\n</style>\n</head>\n<body>\n<main class=\"pageformer\">\n");
    if !main.is_empty() {
        html.push_str(&main);
        html.push('\n');
    }
    html.push_str("</main>\n</body>\n</html>\n");

    Rendered::new(html.into_bytes(), content.warnings)
}

/// The stylesheet a preview can reuse without reparsing the template.
pub fn stylesheet(template: &Template) -> String {
    css::stylesheet(template)
}

/// Why the HTML backend could not be prepared.
#[derive(Debug)]
pub enum HtmlError {
    Asset(AssetError),
}

impl fmt::Display for HtmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for HtmlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
        }
    }
}

impl From<AssetError> for HtmlError {
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Author, Metadata, Title};
    use crate::markdown;

    fn embedded() -> Template {
        Template::embedded().expect("the embedded template must be valid")
    }

    #[test]
    fn a_document_becomes_a_complete_html_file() {
        let document = markdown::parse("# Título\n\nTexto.\n").expect("the document must parse");
        let rendered = render(&document, &embedded(), &Assets::default());
        let html = String::from_utf8_lossy(rendered.bytes());

        assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
        assert!(html.contains("<html>"), "{html}");
        assert!(html.contains("<title></title>"), "{html}");
        assert!(html.contains("<h1>Título</h1>"), "{html}");
        assert!(html.contains("<main class=\"pageformer\">"), "{html}");
        assert!(html.contains("/*<![CDATA[*/"), "{html}");
        assert!(html.ends_with("</html>\n"), "{html}");
        assert!(rendered.warnings().is_empty());
    }

    #[test]
    fn metadata_reaches_the_head() {
        let metadata = Metadata::new(
            Some(Title::parse("Contrato").expect("a title")),
            Some(Author::parse("Pageformer").expect("an author")),
            Some(crate::document::Language::parse("pt-BR").expect("a language")),
        );
        let document = Document::new(metadata, Vec::new());
        let rendered = render(&document, &embedded(), &Assets::default());
        let html = String::from_utf8_lossy(rendered.bytes());

        assert!(html.contains("<html lang=\"pt-BR\">"), "{html}");
        assert!(html.contains("<title>Contrato</title>"), "{html}");
        assert!(
            html.contains("<meta name=\"author\" content=\"Pageformer\" />"),
            "{html}"
        );
    }

    #[test]
    fn the_output_is_deterministic() {
        let document = markdown::parse("Texto.\n").expect("the document must parse");
        let first = render(&document, &embedded(), &Assets::default());
        let second = render(&document, &embedded(), &Assets::default());

        assert_eq!(first.bytes(), second.bytes());
    }

    #[test]
    fn the_public_stylesheet_is_available_to_a_preview() {
        let css = stylesheet(&embedded());

        assert!(css.contains("main.pageformer"), "{css}");
        assert!(css.contains("@media print"), "{css}");
    }
}
