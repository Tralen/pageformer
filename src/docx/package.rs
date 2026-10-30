//! The fixed package parts: content types, relationships and properties.

use std::collections::BTreeSet;

use crate::assets::media_type;
use crate::document::Document;
use crate::xml;

use super::DocxError;
use super::content::{Media, hyperlink_relationship};

const CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
const EXTENDED_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
const STYLES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const HEADER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const FOOTER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

/// The fixed timestamp of the document properties.
const FIXED_DATE: &str = "2026-01-01T00:00:00Z";

/// Generates `[Content_Types].xml`.
pub(crate) fn content_types(
    media: &[Media],
    header: bool,
    footer: bool,
) -> Result<String, DocxError> {
    let mut extensions = BTreeSet::new();
    for entry in media {
        let name = entry.package_path.rsplit('/').next().unwrap_or_default();
        let media_type = media_type(name).ok_or_else(|| DocxError::UnsupportedMediaType {
            name: name.to_owned(),
        })?;
        extensions.insert((extension(name).to_owned(), media_type.to_owned()));
    }
    let mut output = String::new();
    xml::push_fmt(
        &mut output,
        format_args!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"{CONTENT_TYPES}\">\n<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n<Default Extension=\"xml\" ContentType=\"application/xml\"/>\n"
        ),
    );
    for (extension, media_type) in extensions {
        xml::push_fmt(
            &mut output,
            format_args!("<Default Extension=\"{extension}\" ContentType=\"{media_type}\"/>\n"),
        );
    }
    xml::push(
        &mut output,
        "<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\n\
         <Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\n\
         <Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>\n\
         <Override PartName=\"/word/footnotes.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml\"/>\n\
         <Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/>\n",
    );
    if header {
        xml::push(
            &mut output,
            "<Override PartName=\"/word/header1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/>\n",
        );
    }
    if footer {
        xml::push(
            &mut output,
            "<Override PartName=\"/word/footer1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/>\n",
        );
    }
    xml::push(
        &mut output,
        "<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\n\
         <Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\n\
         </Types>\n",
    );
    Ok(output)
}

fn extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((_, extension)) => extension,
        None => "",
    }
}

/// Generates `_rels/.rels`.
pub(crate) fn root_relationships() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{RELATIONSHIPS}\">\n\
         <Relationship Id=\"rId1\" Type=\"{OFFICE_DOCUMENT}\" Target=\"word/document.xml\"/>\n\
         <Relationship Id=\"rId2\" Type=\"{CORE_PROPERTIES}\" Target=\"docProps/core.xml\"/>\n\
         <Relationship Id=\"rId3\" Type=\"{EXTENDED_PROPERTIES}\" Target=\"docProps/app.xml\"/>\n\
         </Relationships>\n"
    )
}

/// Generates `word/_rels/document.xml.rels`.
pub(crate) fn document_relationships(
    media: &[Media],
    hyperlinks: &[String],
    header: bool,
    footer: bool,
) -> String {
    let mut output = String::new();
    xml::push_fmt(
        &mut output,
        format_args!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{RELATIONSHIPS}\">\n\
             <Relationship Id=\"rId1\" Type=\"{STYLES}\" Target=\"styles.xml\"/>\n\
             <Relationship Id=\"rId2\" Type=\"{NUMBERING}\" Target=\"numbering.xml\"/>\n\
             <Relationship Id=\"rId3\" Type=\"{FOOTNOTES}\" Target=\"footnotes.xml\"/>\n\
             <Relationship Id=\"rId4\" Type=\"{SETTINGS}\" Target=\"settings.xml\"/>\n"
        ),
    );
    if header {
        xml::push_fmt(
            &mut output,
            format_args!("<Relationship Id=\"rId5\" Type=\"{HEADER}\" Target=\"header1.xml\"/>\n"),
        );
    }
    if footer {
        xml::push_fmt(
            &mut output,
            format_args!("<Relationship Id=\"rId6\" Type=\"{FOOTER}\" Target=\"footer1.xml\"/>\n"),
        );
    }
    for entry in media {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<Relationship Id=\"{}\" Type=\"{IMAGE}\" Target=\"{}\"/>\n",
                entry.relationship,
                xml::escape_attribute(
                    entry
                        .package_path
                        .strip_prefix("word/")
                        .unwrap_or(&entry.package_path)
                )
            ),
        );
    }
    for (index, destination) in hyperlinks.iter().enumerate() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<Relationship Id=\"{}\" Type=\"{HYPERLINK}\" Target=\"{}\" TargetMode=\"External\"/>\n",
                hyperlink_relationship(media.len(), index.saturating_add(1)),
                xml::escape_attribute(destination)
            ),
        );
    }
    xml::push(&mut output, "</Relationships>\n");
    output
}

/// Generates `word/_rels/header1.xml.rels` when the header carries the
/// letterhead.
pub(crate) fn header_relationships(media: &[Media], letterhead: Option<&str>) -> Option<String> {
    let letterhead = letterhead?;
    let entry = media
        .iter()
        .find(|entry| entry.virtual_name == letterhead)?;
    Some(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{RELATIONSHIPS}\">\n\
         <Relationship Id=\"{}\" Type=\"{IMAGE}\" Target=\"{}\"/>\n\
         </Relationships>\n",
        entry.relationship,
        xml::escape_attribute(
            entry
                .package_path
                .strip_prefix("word/")
                .unwrap_or(&entry.package_path)
        )
    ))
}

/// Generates `docProps/core.xml`. It carries no body text.
pub(crate) fn core_xml(document: &Document) -> String {
    let metadata = document.metadata();
    let mut output = String::new();
    xml::push(
        &mut output,
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    if let Some(title) = metadata.title() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<dc:title>{}</dc:title>\n",
                xml::escape_text(title.as_str())
            ),
        );
    }
    if let Some(author) = metadata.author() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<dc:creator>{}</dc:creator>\n",
                xml::escape_text(author.as_str())
            ),
        );
    }
    if let Some(subject) = metadata.subject() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<dc:subject>{}</dc:subject>\n",
                xml::escape_text(subject.as_str())
            ),
        );
    }
    if !metadata.keywords().is_empty() {
        let keywords = metadata
            .keywords()
            .iter()
            .map(|keyword| keyword.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        xml::push_fmt(
            &mut output,
            format_args!(
                "<cp:keywords>{}</cp:keywords>\n",
                xml::escape_text(&keywords)
            ),
        );
    }
    if let Some(language) = metadata.language() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<dc:language>{}</dc:language>\n",
                xml::escape_text(language.as_str())
            ),
        );
    }
    xml::push_fmt(
        &mut output,
        format_args!(
            "<dcterms:created xsi:type=\"dcterms:W3CDTF\">{FIXED_DATE}</dcterms:created>\n\
             <dcterms:modified xsi:type=\"dcterms:W3CDTF\">{FIXED_DATE}</dcterms:modified>\n\
             </cp:coreProperties>\n"
        ),
    );
    output
}

/// Generates `docProps/app.xml`.
pub(crate) fn app_xml() -> String {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
        "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">\n",
        "<Application>Pageformer</Application>\n",
        "</Properties>\n"
    )
    .to_owned()
}

/// Generates `word/settings.xml`.
pub(crate) fn settings_xml() -> String {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
        "<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n",
        "<w:autoHyphenation w:val=\"true\"/>\n",
        "<w:updateFields w:val=\"true\"/>\n",
        "<w:compat/>\n",
        "</w:settings>\n"
    )
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_content_types_declare_every_part() {
        let types = content_types(&[], true, true).expect("the content types");

        assert!(types.contains("/word/document.xml"));
        assert!(types.contains("/word/header1.xml"));
        assert!(types.contains("/word/footer1.xml"));
        assert!(types.contains("/docProps/core.xml"));
    }

    #[test]
    fn a_media_file_adds_a_default_content_type() {
        let media = vec![Media {
            virtual_name: "assets/image-0.png".to_owned(),
            package_path: "word/media/image-0.png".to_owned(),
            relationship: "rId7".to_owned(),
        }];

        let types = content_types(&media, false, false).expect("the content types");

        assert!(types.contains("<Default Extension=\"png\" ContentType=\"image/png\"/>"));
    }

    #[test]
    fn hyperlinks_are_external_relationships_after_the_media() {
        let media = vec![Media {
            virtual_name: "assets/image-0.png".to_owned(),
            package_path: "word/media/image-0.png".to_owned(),
            relationship: "rId7".to_owned(),
        }];
        let rels =
            document_relationships(&media, &["https://example.org".to_owned()], false, false);

        assert!(rels.contains("Id=\"rId10\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\""));
        assert!(rels.contains("TargetMode=\"External\""));
    }
}
