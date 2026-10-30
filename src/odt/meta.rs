//! Generates `meta.xml` and `META-INF/manifest.xml`.

use crate::assets::Assets;
use crate::document::Document;

use super::{OdtError, media_type, picture_name};
use crate::xml;

const META_PREFIX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" office:version="1.3">
<office:meta>
"#;

const MANIFEST_PREFIX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">
<manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.text"/>
<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>
"#;

const GENERATOR: &str = concat!("Pageformer ", env!("CARGO_PKG_VERSION"));

/// Generates the metadata part. It carries no body text, so a confidential
/// value can never reach it.
pub fn meta_xml(document: &Document) -> String {
    let metadata = document.metadata();
    let mut output = String::new();
    xml::push(&mut output, META_PREFIX);
    xml::push_fmt(
        &mut output,
        format_args!(
            "<meta:generator>{}</meta:generator>\n",
            xml::escape_text(GENERATOR)
        ),
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
    if let Some(language) = metadata.language() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<dc:language>{}</dc:language>\n",
                xml::escape_text(language.as_str())
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
    for keyword in metadata.keywords() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<meta:keyword>{}</meta:keyword>\n",
                xml::escape_text(keyword.as_str())
            ),
        );
    }
    xml::push(&mut output, "</office:meta>\n</office:document-meta>\n");
    output
}

/// Generates the manifest, including one entry per packaged picture.
pub fn manifest_xml(assets: &Assets) -> Result<String, OdtError> {
    let mut output = String::new();
    xml::push(&mut output, MANIFEST_PREFIX);
    for virtual_name in assets.files().keys() {
        let name = picture_name(virtual_name);
        let media = media_type(name).ok_or_else(|| OdtError::UnsupportedMediaType {
            name: name.to_owned(),
        })?;
        xml::push_fmt(
            &mut output,
            format_args!(
                "<manifest:file-entry manifest:full-path=\"Pictures/{name}\" manifest:media-type=\"{media}\"/>\n",
                name = xml::escape_attribute(name),
            ),
        );
    }
    xml::push(&mut output, "</manifest:manifest>\n");
    Ok(output)
}
