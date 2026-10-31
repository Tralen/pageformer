//! Builds the EPUB package parts.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::assets::{Assets, media_type};
use crate::document::{Block, Document, Metadata};
use crate::epub::chapters::{self, ChapterKind};
use crate::epub::{EpubError, MIMETYPE};
use crate::html::content::{self, ImageSource};
use crate::html::css;
use crate::template::Template;
use crate::xml;
use crate::zip::{self, Entry};

/// The package bytes and the warnings the content produced.
pub(crate) struct Built {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

struct RenderedChapter {
    file: String,
    label: String,
    body: String,
}

struct ImageEntry<'a> {
    id: String,
    href: String,
    media_type: &'static str,
    bytes: &'a [u8],
}

/// Builds the complete EPUB package.
pub(crate) fn build(
    document: &Document,
    template: &Template,
    assets: &Assets,
) -> Result<Built, EpubError> {
    let metadata = document.metadata();
    let title = document_title(metadata, document.blocks());
    let language = metadata
        .language()
        .map_or("und", |language| language.as_str());
    let definitions = definitions(document.blocks());
    let mut chapters = Vec::new();
    let mut warnings = Vec::new();

    for chapter in chapters::split(document.blocks()) {
        let mut blocks = chapter.blocks;
        blocks.extend(definitions.iter().cloned());
        let chapter_document = Document::new(metadata.clone(), blocks);
        let content = content::render_with(
            &chapter_document,
            template,
            assets,
            ImageSource::Relative("images/"),
        );
        for warning in content.warnings.clone() {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        let body = content.full();
        if body.is_empty() {
            continue;
        }
        let label = match chapter.kind {
            ChapterKind::Heading(label) => label,
            ChapterKind::Introduction => "Introduction".to_owned(),
            ChapterKind::WholeDocument => title.clone(),
        };
        chapters.push(RenderedChapter {
            file: String::new(),
            label,
            body,
        });
    }

    if chapters.is_empty() {
        chapters.push(RenderedChapter {
            file: String::new(),
            label: title.clone(),
            body: String::new(),
        });
    }
    for (index, chapter) in chapters.iter_mut().enumerate() {
        chapter.file = format!("chapter-{}.xhtml", index.saturating_add(1));
    }

    let identifier = match metadata.identifier() {
        Some(identifier) => identifier.as_str().to_owned(),
        None => derived_identifier(metadata, &chapters),
    };
    let modified = metadata.date().map_or_else(
        || "2026-01-01T00:00:00Z".to_owned(),
        |date| format!("{date}T00:00:00Z"),
    );
    let images = images(assets)?;
    let stylesheet = css::epub_stylesheet(template);
    let navigation = navigation_xhtml(&chapters, title.as_str(), language);
    let package = package_xml(
        metadata,
        &identifier,
        title.as_str(),
        language,
        modified.as_str(),
        &chapters,
        &images,
    );
    let container = container_xml();

    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("mimetype".to_owned(), MIMETYPE.as_bytes().to_vec()),
        ("META-INF/container.xml".to_owned(), container.into_bytes()),
        ("OEBPS/content.opf".to_owned(), package.into_bytes()),
        ("OEBPS/nav.xhtml".to_owned(), navigation.into_bytes()),
        ("OEBPS/style.css".to_owned(), stylesheet.into_bytes()),
    ];
    for chapter in &chapters {
        files.push((
            format!("OEBPS/{}", chapter.file),
            chapter_xhtml(chapter.label.as_str(), chapter.body.as_str(), language).into_bytes(),
        ));
    }
    for image in &images {
        files.push((format!("OEBPS/{}", image.href), image.bytes.to_vec()));
    }

    let entries = files
        .iter()
        .map(|(name, bytes)| Entry::new(name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    let bytes = zip::write_stored(&entries).map_err(EpubError::Package)?;
    Ok(Built { bytes, warnings })
}

fn definitions(blocks: &[Block]) -> Vec<Block> {
    let mut definitions = Vec::new();
    collect_definitions(blocks, &mut definitions);
    definitions
}

fn collect_definitions(blocks: &[Block], definitions: &mut Vec<Block>) {
    for block in blocks {
        match block {
            Block::FootnoteDefinition(definition) => {
                definitions.push(block.clone());
                collect_definitions(definition.blocks(), definitions);
            }
            Block::List(list) => {
                for item in list.items() {
                    collect_definitions(item.blocks(), definitions);
                }
            }
            Block::Quote(quote) => collect_definitions(quote.blocks(), definitions),
            Block::Heading(_)
            | Block::Paragraph(_)
            | Block::NumberedParagraph(_)
            | Block::Table(_)
            | Block::CodeBlock(_)
            | Block::ThematicBreak
            | Block::PageBreak => {}
        }
    }
}

fn document_title(metadata: &Metadata, blocks: &[Block]) -> String {
    if let Some(title) = metadata.title() {
        return title.as_str().to_owned();
    }
    first_heading(blocks).unwrap_or_else(|| "Untitled".to_owned())
}

fn first_heading(blocks: &[Block]) -> Option<String> {
    for block in blocks {
        match block {
            Block::Heading(heading) if heading.level().get() == 1 => {
                let text = content::plain_text(heading.content());
                return (!text.is_empty()).then_some(text);
            }
            Block::List(list) => {
                for item in list.items() {
                    if let Some(text) = first_heading(item.blocks()) {
                        return Some(text);
                    }
                }
            }
            Block::Quote(quote) => {
                if let Some(text) = first_heading(quote.blocks()) {
                    return Some(text);
                }
            }
            Block::FootnoteDefinition(definition) => {
                if let Some(text) = first_heading(definition.blocks()) {
                    return Some(text);
                }
            }
            Block::Paragraph(_)
            | Block::NumberedParagraph(_)
            | Block::Table(_)
            | Block::CodeBlock(_)
            | Block::ThematicBreak
            | Block::PageBreak
            | Block::Heading(_) => {}
        }
    }
    None
}

fn derived_identifier(metadata: &Metadata, chapters: &[RenderedChapter]) -> String {
    let mut identity = String::new();
    push_identity(
        &mut identity,
        "title",
        metadata.title().map_or("", |value| value.as_str()),
    );
    push_identity(
        &mut identity,
        "author",
        metadata.author().map_or("", |value| value.as_str()),
    );
    push_identity(
        &mut identity,
        "language",
        metadata.language().map_or("", |value| value.as_str()),
    );
    push_identity(
        &mut identity,
        "date",
        &metadata
            .date()
            .map_or_else(String::new, |value| value.to_string()),
    );
    push_identity(
        &mut identity,
        "subject",
        metadata.subject().map_or("", |value| value.as_str()),
    );
    push_identity(
        &mut identity,
        "keywords",
        &metadata
            .keywords()
            .iter()
            .map(|keyword| keyword.as_str())
            .collect::<Vec<_>>()
            .join("\u{1f}"),
    );
    for chapter in chapters {
        push_identity(&mut identity, "chapter-label", chapter.label.as_str());
        push_identity(&mut identity, "chapter-body", chapter.body.as_str());
    }
    Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes()).to_string()
}

fn push_identity(identity: &mut String, label: &str, value: &str) {
    identity.push_str(label);
    identity.push(':');
    identity.push_str(&value.len().to_string());
    identity.push(':');
    identity.push_str(value);
    identity.push('\n');
}

fn images(assets: &Assets) -> Result<Vec<ImageEntry<'_>>, EpubError> {
    let mut images: BTreeMap<String, (&'static str, &[u8])> = BTreeMap::new();
    for (_, asset) in assets.map().images() {
        let name = asset.rsplit_once('/').map_or(asset, |(_, name)| name);
        let media = media_type(asset).ok_or_else(|| EpubError::UnsupportedImage {
            name: name.to_owned(),
        })?;
        let bytes = assets
            .files()
            .get(asset)
            .ok_or_else(|| EpubError::MissingImage {
                name: name.to_owned(),
            })?;
        images.insert(format!("images/{name}"), (media, bytes.as_slice()));
    }
    Ok(images
        .into_iter()
        .enumerate()
        .map(|(index, (href, (media_type, bytes)))| ImageEntry {
            id: format!("image-{}", index.saturating_add(1)),
            href,
            media_type,
            bytes,
        })
        .collect())
}

fn container_xml() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
     <container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
     <rootfiles>\n\
     <rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n\
     </rootfiles>\n\
     </container>\n"
        .to_owned()
}

fn package_xml(
    metadata: &Metadata,
    identifier: &str,
    title: &str,
    language: &str,
    modified: &str,
    chapters: &[RenderedChapter],
    images: &[ImageEntry<'_>],
) -> String {
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str(&format!(
        "<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"pub-id\" xml:lang=\"{}\">\n",
        xml::escape_attribute(language)
    ));
    output.push_str("<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n");
    output.push_str(&format!(
        "<dc:identifier id=\"pub-id\">{}</dc:identifier>\n",
        xml::escape_text(identifier)
    ));
    output.push_str(&format!(
        "<dc:title>{}</dc:title>\n",
        xml::escape_text(title)
    ));
    output.push_str(&format!(
        "<dc:language>{}</dc:language>\n",
        xml::escape_text(language)
    ));
    if let Some(author) = metadata.author() {
        output.push_str(&format!(
            "<dc:creator>{}</dc:creator>\n",
            xml::escape_text(author.as_str())
        ));
    }
    if let Some(date) = metadata.date() {
        output.push_str(&format!(
            "<dc:date>{}</dc:date>\n",
            xml::escape_text(&date.to_string())
        ));
    }
    if let Some(subject) = metadata.subject() {
        output.push_str(&format!(
            "<dc:subject>{}</dc:subject>\n",
            xml::escape_text(subject.as_str())
        ));
    }
    for keyword in metadata.keywords() {
        output.push_str(&format!(
            "<dc:subject>{}</dc:subject>\n",
            xml::escape_text(keyword.as_str())
        ));
    }
    output.push_str(&format!(
        "<meta property=\"dcterms:modified\">{}</meta>\n",
        xml::escape_text(modified)
    ));
    output.push_str("</metadata>\n<manifest>\n");
    output.push_str(
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n",
    );
    output.push_str("<item id=\"style\" href=\"style.css\" media-type=\"text/css\"/>\n");
    for chapter in chapters {
        output.push_str(&format!(
            "<item id=\"{}\" href=\"{}\" media-type=\"application/xhtml+xml\"/>\n",
            xml::escape_attribute(chapter.file.trim_end_matches(".xhtml")),
            xml::escape_attribute(chapter.file.as_str())
        ));
    }
    for image in images {
        output.push_str(&format!(
            "<item id=\"{}\" href=\"{}\" media-type=\"{}\"/>\n",
            xml::escape_attribute(image.id.as_str()),
            xml::escape_attribute(image.href.as_str()),
            xml::escape_attribute(image.media_type)
        ));
    }
    output.push_str("</manifest>\n<spine>\n");
    output.push_str("<itemref idref=\"nav\" linear=\"no\"/>\n");
    for chapter in chapters {
        output.push_str(&format!(
            "<itemref idref=\"{}\"/>\n",
            xml::escape_attribute(chapter.file.trim_end_matches(".xhtml"))
        ));
    }
    output.push_str("</spine>\n</package>\n");
    output
}

fn navigation_xhtml(chapters: &[RenderedChapter], title: &str, language: &str) -> String {
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<!DOCTYPE html>\n");
    output.push_str(&format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{}\" xml:lang=\"{}\">\n",
        xml::escape_attribute(language),
        xml::escape_attribute(language)
    ));
    output.push_str("<head>\n<meta charset=\"utf-8\" />\n");
    output.push_str(&format!("<title>{}</title>\n", xml::escape_text(title)));
    output.push_str(
        "<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\" />\n</head>\n<body>\n",
    );
    output.push_str("<nav epub:type=\"toc\" id=\"toc\">\n<h1>Contents</h1>\n<ol>\n");
    for chapter in chapters {
        output.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>\n",
            xml::escape_attribute(chapter.file.as_str()),
            xml::escape_text(chapter.label.as_str())
        ));
    }
    output.push_str("</ol>\n</nav>\n</body>\n</html>\n");
    output
}

fn chapter_xhtml(title: &str, body: &str, language: &str) -> String {
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<!DOCTYPE html>\n");
    output.push_str(&format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\" lang=\"{}\" xml:lang=\"{}\">\n",
        xml::escape_attribute(language),
        xml::escape_attribute(language)
    ));
    output.push_str("<head>\n<meta charset=\"utf-8\" />\n");
    output.push_str(&format!("<title>{}</title>\n", xml::escape_text(title)));
    output.push_str("<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\" />\n</head>\n<body>\n<main class=\"pageformer\">\n");
    output.push_str(body);
    if !body.is_empty() {
        output.push('\n');
    }
    output.push_str("</main>\n</body>\n</html>\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Author, Language, Metadata, Title};

    #[test]
    fn the_container_names_the_package_document() {
        let container = container_xml();

        assert!(container.contains("OEBPS/content.opf"), "{container}");
        assert!(
            container.contains("application/oebps-package+xml"),
            "{container}"
        );
    }

    #[test]
    fn the_package_carries_the_navigation_and_the_spine() {
        let metadata = Metadata::new(
            Some(Title::parse("Contrato").expect("a title")),
            Some(Author::parse("Pageformer").expect("an author")),
            Some(Language::parse("pt-BR").expect("a language")),
        );
        let chapters = vec![RenderedChapter {
            file: "chapter-1.xhtml".to_owned(),
            label: "Contrato".to_owned(),
            body: "<p>Texto.</p>".to_owned(),
        }];
        let package = package_xml(
            &metadata,
            "urn:uuid:00000000-0000-0000-0000-000000000000",
            "Contrato",
            "pt-BR",
            "2026-01-01T00:00:00Z",
            &chapters,
            &[],
        );

        assert!(package.contains("properties=\"nav\""), "{package}");
        assert!(
            package.contains("<itemref idref=\"nav\" linear=\"no\"/>"),
            "{package}"
        );
        assert!(
            package.contains("<itemref idref=\"chapter-1\"/>"),
            "{package}"
        );
        assert!(
            package.contains("<meta property=\"dcterms:modified\">2026-01-01T00:00:00Z</meta>"),
            "{package}"
        );
    }

    #[test]
    fn the_navigation_lists_every_chapter() {
        let chapters = vec![
            RenderedChapter {
                file: "chapter-1.xhtml".to_owned(),
                label: "Um".to_owned(),
                body: String::new(),
            },
            RenderedChapter {
                file: "chapter-2.xhtml".to_owned(),
                label: "Dois".to_owned(),
                body: String::new(),
            },
        ];
        let nav = navigation_xhtml(&chapters, "Livro", "pt-BR");

        assert!(nav.contains("epub:type=\"toc\""), "{nav}");
        assert!(nav.contains("href=\"chapter-1.xhtml\">Um</a>"), "{nav}");
        assert!(nav.contains("href=\"chapter-2.xhtml\">Dois</a>"), "{nav}");
    }
}
