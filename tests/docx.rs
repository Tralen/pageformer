//! Structural tests for the DOCX backend.

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use pageformer::assets::Assets;
use pageformer::confidential::loader as confidential_loader;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::document::{Block, Destination, Document, Inline, Metadata};
use pageformer::docx;
use pageformer::markdown;
use pageformer::template::Template;
use tempfile::TempDir;
use zip::ZipArchive;

const CONTRACT: &str = include_str!("../assets/examples/escritorio/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/escritorio/Embargos.md");
const REPORT: &str = include_str!("../assets/examples/tecnico/Relatorio.md");
const EMBARGOS_VALUES: &str = include_str!("../assets/examples/confidencial/embargos.ini");

fn embedded() -> Template {
    Template::embedded().expect("the embedded template must be valid")
}

fn example_store() -> ConfidentialStore {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("embargos.ini");
    fs::write(&path, EMBARGOS_VALUES).expect("the values must be written");
    make_private(&path);
    confidential_loader::load(Some(&path), &[]).expect("the example values must load")
}

#[cfg(unix)]
fn make_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .expect("the permissions must change");
}

#[cfg(not(unix))]
fn make_private(_path: &Path) {}

fn render(document: &Document) -> Vec<u8> {
    docx::render(document, &embedded(), &Assets::default()).expect("the package must be produced")
}

fn entry(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> String {
    let mut text = String::new();
    archive
        .by_name(name)
        .expect("the entry must exist")
        .read_to_string(&mut text)
        .expect("the entry must be UTF-8");
    text
}

fn names(archive: &mut ZipArchive<Cursor<Vec<u8>>>) -> Vec<String> {
    archive
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.into_owned())
        .collect()
}

#[test]
fn the_package_has_the_required_parts_and_every_xml_parses() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join("escritorio");
    let resolved = pageformer::template::loader::resolve(&examples, None)
        .expect("the example template must resolve");
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let assets = pageformer::assets::load_assets(
        &document,
        resolved.template(),
        &examples,
        resolved.base_directory(),
    )
    .expect("the assets must load");
    let bytes = docx::render(&document, resolved.template(), &assets).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let names = names(&mut archive);

    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/styles.xml",
        "word/numbering.xml",
        "word/footnotes.xml",
        "word/settings.xml",
        "word/header1.xml",
        "word/_rels/header1.xml.rels",
        "word/media/letterhead.png",
    ] {
        assert!(
            names.iter().any(|name| name == required),
            "missing {required}"
        );
    }
    for name in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/styles.xml",
        "word/numbering.xml",
        "word/footnotes.xml",
        "word/settings.xml",
        "word/header1.xml",
        "word/_rels/header1.xml.rels",
    ] {
        let text = entry(&mut archive, name);
        roxmltree::Document::parse(&text)
            .unwrap_or_else(|error| panic!("{name} is not well formed: {error}"));
    }

    let content = entry(&mut archive, "word/document.xml");
    assert!(content.contains("Das partes"), "{content}");
    assert!(content.contains("<w:tbl>"), "tables must survive");
    assert!(content.contains("<w:tblHeader/>"), "the header row repeats");
    assert!(content.contains("<w:numPr>"), "lists use numbering");
    assert!(content.contains("w:header=\"530\""), "the header sits high");
    assert!(
        content.contains("<w:spacing w:after=\"0\" w:line="),
        "cells do not inherit the paragraph spacing"
    );
    let header = entry(&mut archive, "word/header1.xml");
    assert!(header.contains("behindDoc=\"1\""), "the letterhead floats");
    let settings = entry(&mut archive, "word/settings.xml");
    assert!(
        !settings.contains("autoHyphenation"),
        "hyphenation stays with the editor"
    );
    let styles = entry(&mut archive, "word/styles.xml");
    assert!(
        styles.contains("w:lineRule=\"atLeast\""),
        "a tight line does not clip the title"
    );
    assert!(!styles.contains("w:lineRule=\"exact\""), "{styles}");
}

#[test]
fn the_same_document_produces_the_same_bytes() {
    let document = markdown::parse(REPORT).expect("the report fixture must parse");

    assert_eq!(
        docx::render(&document, &embedded(), &Assets::default()).expect("the package"),
        docx::render(&document, &embedded(), &Assets::default()).expect("the package")
    );
}

#[test]
fn the_embargos_metadata_never_carries_a_value() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");

    let bytes = docx::render(resolution.document(), &embedded(), &Assets::default())
        .expect("the package must be produced");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let core = entry(&mut archive, "docProps/core.xml");
    let content = entry(&mut archive, "word/document.xml");

    assert!(content.contains("55.444.333"), "the body carries the value");
    assert!(
        !core.contains("55.444.333"),
        "a confidential value reached the properties"
    );
}

#[test]
fn a_template_change_is_visible_in_the_styles() {
    let document = Document::new(
        Metadata::default(),
        vec![Block::Paragraph(vec![Inline::Text("Texto".to_owned())])],
    );
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  paragraph:
    size: 13
    leading: 4
  heading:
    h1:
      size: 27
  table:
    header_fill: "#123456"
    border: 2pt
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "word/styles.xml");

    assert!(styles.contains("<w:sz w:val=\"26\"/>"), "{styles}");
    assert!(styles.contains("<w:sz w:val=\"54\"/>"), "{styles}");
}

#[test]
fn a_rule_and_a_heading_background_follow_the_template() {
    let document =
        markdown::parse("# Título\n\nTexto.\n\n---\n\nDepois.\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  heading:
    background: "#EEEEEE"
  rule:
    thickness: 1.5pt
    color: "#112233"
    spacing_before: 12
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "word/styles.xml");
    let content = entry(&mut archive, "word/document.xml");

    assert!(
        styles.contains("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"eeeeee\"/>"),
        "{styles}"
    );
    assert!(
        content.contains(
            "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"12\" w:space=\"1\" w:color=\"112233\"/></w:pBdr><w:spacing w:before=\"240\"/>"
        ),
        "{content}"
    );
}

#[test]
fn the_foreground_reaches_the_styles_and_the_runs() {
    let document = markdown::parse("## Escopo\n\nTexto com *ênfase* e **forte**.\n")
        .expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  paragraph:
    foreground: "#112233"
    emphasis:
      foreground: "#445566"
  heading:
    foreground: "#778899"
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "word/styles.xml");
    let content = entry(&mut archive, "word/document.xml");

    assert!(
        styles.contains("<w:rPrDefault><w:rPr><w:rFonts"),
        "the defaults carry the body color: {styles}"
    );
    assert_eq!(
        styles.matches("<w:color w:val=\"112233\"/>").count(),
        2,
        "the document defaults and Normal take the body color: {styles}"
    );
    assert!(
        styles.contains("<w:color w:val=\"778899\"/>"),
        "headings take their own color: {styles}"
    );
    assert!(
        content.contains("<w:i/><w:color w:val=\"445566\"/>"),
        "emphasis overrides the body color: {content}"
    );
    assert!(
        !content.contains("<w:b/><w:color"),
        "strong inherits the body color instead of repeating it: {content}"
    );
}

#[test]
fn the_paragraph_and_block_styles_reach_the_package() {
    let document = markdown::parse(
        "# Título\n\nTexto com `código` e ~~riscado~~.\n\n> Citação.\n\n```rust\nfn main() {}\n```\n",
    )
    .expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  paragraph:
    background: "#fafafa"
    code:
      font_families:
        - DejaVu Sans Mono
      foreground: navy
      background: "#eeeeee"
    strikethrough:
      foreground: crimson
  quote:
    background: "#f0f0f0"
    border:
      left: 3pt
      color: "#cccccc"
      padding: 8
  code_block:
    background: "#111111"
    foreground: "#eeeeee"
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "word/styles.xml");
    let content = entry(&mut archive, "word/document.xml");

    assert!(
        styles.contains(
            "<w:style w:type=\"paragraph\" w:styleId=\"BodyParagraph\"><w:name w:val=\"Body Paragraph\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"fafafa\"/></w:pPr></w:style>"
        ),
        "the body paragraph carries the fill through its own style: {styles}"
    );
    let code_char = styles
        .split("w:styleId=\"CodeChar\"")
        .nth(1)
        .expect("the inline code style");
    assert!(
        code_char.contains("<w:color w:val=\"000080\"/>"),
        "{styles}"
    );
    assert!(
        code_char.contains("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"eeeeee\"/>"),
        "{styles}"
    );
    assert!(
        code_char.contains("w:ascii=\"DejaVu Sans Mono\""),
        "an inline style changes its own font: {styles}"
    );
    let quote = styles
        .split("w:styleId=\"Quote\"")
        .nth(1)
        .expect("the quote style");
    assert!(
        quote.contains("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"f0f0f0\"/>"),
        "{styles}"
    );
    assert!(
        quote.contains("<w:left w:val=\"single\" w:sz=\"24\" w:space=\"8\" w:color=\"cccccc\"/>")
            && quote.contains("<w:jc w:val=\"left\"/>"),
        "the quote carries its left border and left alignment: {styles}"
    );
    let code = styles
        .split("w:styleId=\"Code\"")
        .nth(1)
        .expect("the code style");
    assert!(
        code.contains("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"111111\"/>"),
        "{styles}"
    );
    assert!(
        content.contains("<w:strike/>")
            && content.contains("<w:color w:val=\"dc143c\"/>")
            && content.contains("w:ascii=\"Liberation Serif\""),
        "the strikethrough run takes the color and the paragraph face: {content}"
    );
}

#[test]
fn a_heading_rule_is_a_bottom_border_of_its_level() {
    let document = markdown::parse("## Escopo\n\nTexto.\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  heading:
    h2:
      rule:
        thickness: 1.5pt
        color: "#112233"
        spacing_before: 3
        spacing_after: 24
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "word/styles.xml");

    assert!(
        styles.contains(
            "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"12\" w:space=\"3\" w:color=\"112233\"/></w:pBdr>"
        ),
        "{styles}"
    );
    assert!(styles.contains("w:after=\"280\""), "{styles}");
    assert_eq!(styles.matches("<w:pBdr>").count(), 1, "{styles}");
}

#[test]
fn two_lists_get_their_own_numbering_and_a_restart() {
    let source = "- a\n- b\n\n3. c\n4. d\n";
    let document = markdown::parse(source).expect("the document must parse");
    let bytes = render(&document);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let numbering = entry(&mut archive, "word/numbering.xml");
    let content = entry(&mut archive, "word/document.xml");

    assert_eq!(numbering.matches("<w:num ").count(), 2, "{numbering}");
    assert!(
        numbering.contains("<w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"3\"/>"),
        "{numbering}"
    );
    assert!(content.contains("<w:numId w:val=\"1\"/>"), "{content}");
    assert!(content.contains("<w:numId w:val=\"2\"/>"), "{content}");
}

#[test]
fn lists_take_their_markers_layout_and_text_from_the_template() {
    let document = markdown::parse("- Um\n    - Dois\n\n- Três\n\n1. Primeiro\n    1. Interno\n")
        .expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  list:
    foreground: "#112233"
    background: "#eeeeee"
    alignment: justified
    indent: 6mm
    marker_gap: 3mm
    bullet:
      - "*"
      - "+"
    numbering:
      format:
        - upper-roman
        - upper-alpha
      suffix: ")"
    spacing: 12pt
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let numbering = entry(&mut archive, "word/numbering.xml");
    let styles = entry(&mut archive, "word/styles.xml");
    let content = entry(&mut archive, "word/document.xml");

    assert!(
        numbering.contains("<w:lvlText w:val=\"*\"/>")
            && numbering.contains("<w:lvlText w:val=\"+\"/>"),
        "the bullets follow the template: {numbering}"
    );
    assert!(
        numbering.contains("<w:numFmt w:val=\"upperRoman\"/>")
            && numbering.contains("<w:lvlText w:val=\"%1)\"/>")
            && numbering.contains("<w:numFmt w:val=\"upperLetter\"/>")
            && numbering.contains("<w:lvlText w:val=\"%2)\"/>"),
        "the ordered levels use the template formats and suffix: {numbering}"
    );
    assert!(
        numbering.contains("<w:ind w:left=\"510\" w:hanging=\"170\"/>")
            && numbering.contains("<w:ind w:left=\"850\" w:hanging=\"170\"/>"),
        "the levels use the template indent and gap: {numbering}"
    );
    assert!(
        numbering.contains("<w:color w:val=\"112233\"/>"),
        "the marker inherits the list color: {numbering}"
    );
    assert!(
        styles.contains("w:styleId=\"ListParagraph\"")
            && styles.contains("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"eeeeee\"/>")
            && styles.contains("<w:jc w:val=\"both\"/>")
            && styles.contains("<w:spacing w:after=\"84\""),
        "the list paragraph follows the template: {styles}"
    );
    assert!(
        content.contains("<w:pStyle w:val=\"ListParagraph\"/>"),
        "every list item uses the list paragraph style: {content}"
    );
}

#[test]
fn footnotes_and_links_are_packaged() {
    let source = "Texto[^1] e [site](https://example.org).\n\n[^1]: Nota.\n";
    let document = markdown::parse(source).expect("the document must parse");
    let bytes = render(&document);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let content = entry(&mut archive, "word/document.xml");
    let footnotes = entry(&mut archive, "word/footnotes.xml");
    let relationships = entry(&mut archive, "word/_rels/document.xml.rels");

    assert!(
        content.contains("<w:footnoteReference w:id=\"1\"/>"),
        "{content}"
    );
    assert!(footnotes.contains("<w:footnote w:id=\"1\">"), "{footnotes}");
    assert!(footnotes.contains("<w:footnoteRef/>"), "{footnotes}");
    assert!(content.contains("<w:hyperlink r:id="), "{content}");
    assert!(
        relationships.contains("TargetMode=\"External\""),
        "{relationships}"
    );
    assert!(
        relationships.contains("https://example.org"),
        "{relationships}"
    );
}

#[test]
fn links_take_the_template_style() {
    let document =
        markdown::parse("Veja [o site](https://example.org).\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.6"
formatting:
  paragraph:
    link:
      foreground: "#112233"
      underline: true
"##,
    )
    .expect("the template must be valid");

    let bytes = docx::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "word/styles.xml");
    let content = entry(&mut archive, "word/document.xml");
    let hyperlink = styles
        .split("w:styleId=\"Hyperlink\"")
        .nth(1)
        .expect("the hyperlink character style");

    assert!(
        hyperlink.contains("<w:color w:val=\"112233\"/>"),
        "{styles}"
    );
    assert!(hyperlink.contains("<w:u w:val=\"single\"/>"), "{styles}");
    assert!(
        content.contains("<w:rStyle w:val=\"Hyperlink\"/>"),
        "{content}"
    );
}

#[test]
fn an_inline_image_is_packaged_and_referenced() {
    let project = TempDir::new().expect("a temporary project");
    fs::write(project.path().join("logo.png"), minimal_png()).expect("the image must be written");
    let document = Document::new(
        Metadata::default(),
        vec![Block::Paragraph(vec![Inline::Image {
            destination: Destination::new("logo.png"),
            title: None,
            alternative: vec![Inline::Text("Logo".to_owned())],
        }])],
    );
    let assets = pageformer::assets::load_assets(&document, &embedded(), project.path(), None)
        .expect("the assets must load");

    let bytes = docx::render(&document, &embedded(), &assets).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let names = names(&mut archive);
    let content = entry(&mut archive, "word/document.xml");
    let relationships = entry(&mut archive, "word/_rels/document.xml.rels");

    assert!(
        names.iter().any(|name| name == "word/media/image-0.png"),
        "{names:?}"
    );
    assert!(content.contains("r:embed=\"rId7\""), "{content}");
    assert!(content.contains("descr=\"Logo\""), "{content}");
    assert!(
        relationships.contains("Target=\"media/image-0.png\""),
        "{relationships}"
    );
}

fn minimal_png() -> Vec<u8> {
    vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00,
    ]
}
