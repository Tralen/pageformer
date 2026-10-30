//! Structural tests for the ODT backend.

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use pageformer::assets::Assets;
use pageformer::confidential::loader as confidential_loader;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::document::{
    Block, ColumnAlignment, Destination, Document, Inline, Metadata, Table, TableCell,
};
use pageformer::markdown;
use pageformer::odt;
use pageformer::template::Template;
use tempfile::TempDir;
use zip::CompressionMethod;
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

fn render(body: &Document) -> Vec<u8> {
    odt::render(body, &embedded(), &Assets::default()).expect("the package must be produced")
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

#[test]
fn the_package_is_a_stored_zip_with_the_mimetype_first() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let bytes = render(&document);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");

    let names = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.into_owned())
        .collect::<Vec<_>>();
    assert_eq!(names.first().map(String::as_str), Some("mimetype"));
    assert_eq!(
        archive.by_index(0).expect("the first entry").compression(),
        CompressionMethod::Stored
    );
    let mimetype = entry(&mut archive, "mimetype");
    assert_eq!(mimetype, odt::MIMETYPE);
    for required in [
        "content.xml",
        "styles.xml",
        "meta.xml",
        "META-INF/manifest.xml",
    ] {
        assert!(
            names.iter().any(|name| name == required),
            "missing {required}"
        );
    }
}

#[test]
fn every_xml_part_parses_and_carries_the_expected_content() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let bytes = render(&document);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");

    for name in [
        "content.xml",
        "styles.xml",
        "meta.xml",
        "META-INF/manifest.xml",
    ] {
        let text = entry(&mut archive, name);
        roxmltree::Document::parse(&text)
            .unwrap_or_else(|error| panic!("{name} is not well formed: {error}"));
    }

    let content = entry(&mut archive, "content.xml");
    assert!(content.contains("Das partes"), "{content}");
    assert!(content.contains("<text:h"), "headings must use text:h");
    assert!(content.contains("<table:table"), "tables must survive");
}

#[test]
fn the_same_document_produces_the_same_bytes() {
    let document = markdown::parse(REPORT).expect("the report fixture must parse");

    assert_eq!(
        odt::render(&document, &embedded(), &Assets::default()).expect("the package"),
        odt::render(&document, &embedded(), &Assets::default()).expect("the package")
    );
}

#[test]
fn the_embargos_package_keeps_values_out_of_the_metadata() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");

    let bytes = odt::render(resolution.document(), &embedded(), &Assets::default())
        .expect("the package must be produced");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let metadata = entry(&mut archive, "meta.xml");
    let content = entry(&mut archive, "content.xml");

    assert!(content.contains("55.444.333"), "the body carries the value");
    assert!(
        !metadata.contains("55.444.333"),
        "a confidential value reached the metadata"
    );
}

#[test]
fn a_template_change_is_visible_in_the_styles() {
    let document = Document::new(
        Metadata::default(),
        vec![
            Block::Paragraph(vec![Inline::Text("Texto".to_owned())]),
            Block::Table(
                Table::new(
                    vec![ColumnAlignment::Default],
                    vec![TableCell::new(vec![Inline::Text("H".to_owned())])],
                    vec![],
                )
                .expect("a table"),
            ),
        ],
    );
    let template = Template::from_yaml(
        r##"
version: "0.5"
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

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");
    let content = entry(&mut archive, "content.xml");

    assert!(styles.contains("fo:font-size=\"13.00pt\""), "{styles}");
    assert!(styles.contains("fo:font-size=\"27.00pt\""), "{styles}");
    assert!(styles.contains("fo:line-height=\"17.00pt\""), "{styles}");
    assert!(content.contains("#123456"), "{content}");
    assert!(content.contains("2.00pt"), "{content}");
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

    let bytes = odt::render(&document, &embedded(), &assets).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let names = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.into_owned())
        .collect::<Vec<_>>();
    let content = entry(&mut archive, "content.xml");

    assert!(
        names.iter().any(|name| name == "Pictures/image-0.png"),
        "{names:?}"
    );
    assert!(content.contains("Pictures/image-0.png"), "{content}");
    assert!(content.contains("<svg:desc>Logo</svg:desc>"), "{content}");
}

#[test]
fn footnotes_alerts_and_page_breaks_are_mapped() {
    let source = "Texto[^1].\n\n[^1]: Nota.\n\n{pagebreak}\n\n> [!WARNING]\n> Cuidado.\n";
    let document = markdown::parse(source).expect("the document must parse");
    let bytes = render(&document);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let content = entry(&mut archive, "content.xml");

    assert!(
        content.contains("<text:note text:note-class=\"footnote\""),
        "{content}"
    );
    assert!(
        content.contains("<text:note-citation>1</text:note-citation>"),
        "{content}"
    );
    assert!(content.contains("[!WARNING]"), "{content}");
    assert!(
        content.contains("text:style-name=\"PageBreak\""),
        "{content}"
    );
}

fn minimal_png() -> Vec<u8> {
    // A 1x1 transparent PNG, enough for the dimension reader.
    vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00,
    ]
}
