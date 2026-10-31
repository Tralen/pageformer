//! Structural and content tests for the EPUB backend.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use pageformer::assets::load_images;
use pageformer::confidential::loader as confidential_loader;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::document::{Block, Destination, Document, Inline, Metadata};
use pageformer::epub;
use pageformer::markdown;
use pageformer::template::loader;
use tempfile::TempDir;
use zip::CompressionMethod;
use zip::ZipArchive;

const CONTRACT: &str = include_str!("../assets/examples/escritorio/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/escritorio/Embargos.md");
const REPORT: &str = include_str!("../assets/examples/tecnico/Relatorio.md");
const EMBARGOS_VALUES: &str = include_str!("../assets/examples/confidencial/embargos.ini");
const CONTRACT_EPUB: &[u8] = include_bytes!("../assets/examples/publicacao/Contrato.epub");
const EMBARGOS_EPUB: &[u8] = include_bytes!("../assets/examples/publicacao/Embargos.epub");
const REPORT_EPUB: &[u8] = include_bytes!("../assets/examples/tecnico/Relatorio.epub");

fn example_directory(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join(name)
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

fn render_example(directory: &str, source: &str) -> (Vec<u8>, Vec<String>) {
    let directory = example_directory(directory);
    let resolved = loader::resolve(&directory, None).expect("the example template must resolve");
    let document = markdown::parse(source).expect("the fixture must parse");
    let assets = load_images(&document, &directory).expect("the example images must load");
    let rendered = epub::render(&document, resolved.template(), &assets)
        .expect("the package must be produced");
    let warnings = rendered.warnings().to_vec();
    (rendered.into_bytes(), warnings)
}

fn render_embargos() -> (Vec<u8>, Vec<String>) {
    let directory = example_directory("escritorio");
    let resolved = loader::resolve(&directory, None).expect("the template must resolve");
    let document = markdown::parse(EMBARGOS).expect("the fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");
    let assets = load_images(resolution.document(), &directory).expect("the images must load");
    let rendered = epub::render(resolution.document(), resolved.template(), &assets)
        .expect("the package must be produced");
    let warnings = rendered.warnings().to_vec();
    (rendered.into_bytes(), warnings)
}

fn archive(bytes: Vec<u8>) -> ZipArchive<Cursor<Vec<u8>>> {
    ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP")
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
fn a_page_background_reaches_the_reflowable_body() {
    let template = pageformer::template::Template::from_yaml(
        "version: \"0.7\"\npage:\n  background: \"#fafafa\"\n",
    )
    .expect("the template must be valid");
    let document = markdown::parse("Texto.\n").expect("the document must parse");
    let rendered = epub::render(&document, &template, &pageformer::assets::Assets::default())
        .expect("the package must be produced");
    let mut archive = archive(rendered.into_bytes());
    let css = entry(&mut archive, "OEBPS/style.css");

    assert!(
        css.contains("html, body {\n  background-color: #fafafa;\n}"),
        "the reflowable body carries the fill: {css}"
    );
}

#[test]
fn the_package_starts_with_a_stored_mimetype() {
    let (bytes, warnings) = render_example("escritorio", CONTRACT);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut archive = archive(bytes);
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
    assert_eq!(entry(&mut archive, "mimetype"), epub::MIMETYPE);
    for required in [
        "META-INF/container.xml",
        "OEBPS/content.opf",
        "OEBPS/nav.xhtml",
        "OEBPS/style.css",
    ] {
        assert!(
            names.iter().any(|name| name == required),
            "missing {required}"
        );
    }
}

#[test]
fn every_xml_part_parses() {
    let (bytes, _) = render_example("tecnico", REPORT);
    let mut archive = archive(bytes);
    let names = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.into_owned())
        .collect::<Vec<_>>();

    for name in names {
        if [
            "META-INF/container.xml",
            "OEBPS/content.opf",
            "OEBPS/nav.xhtml",
        ]
        .contains(&name.as_str())
            || name.starts_with("OEBPS/chapter-")
        {
            let text = entry(&mut archive, &name);
            let body = text
                .split_once("<!DOCTYPE html>\n")
                .map_or(text.as_str(), |(_, body)| body);
            roxmltree::Document::parse(body)
                .unwrap_or_else(|error| panic!("{name} is not well formed: {error}"));
        }
    }
}

#[test]
fn the_report_splits_at_first_level_headings() {
    let (bytes, warnings) = render_example("tecnico", REPORT);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut archive = archive(bytes);
    let package = entry(&mut archive, "OEBPS/content.opf");
    let nav = entry(&mut archive, "OEBPS/nav.xhtml");
    let first = entry(&mut archive, "OEBPS/chapter-1.xhtml");

    for chapter in ["chapter-1", "chapter-2", "chapter-3"] {
        assert!(
            package.contains(&format!("<itemref idref=\"{chapter}\"/>")),
            "{package}"
        );
    }
    assert!(nav.contains(">Relatório Técnico</a>"), "{nav}");
    assert!(nav.contains(">Anexo</a>"), "{nav}");
    assert!(nav.contains(">Apêndice</a>"), "{nav}");
    assert!(first.contains("<h1>Relatório Técnico</h1>"), "{first}");
    assert!(first.contains("role=\"doc-endnotes\""), "{first}");
}

#[test]
fn the_two_renders_are_byte_identical() {
    let (first, _) = render_example("tecnico", REPORT);
    let (second, _) = render_example("tecnico", REPORT);

    assert_eq!(first, second);
}

#[test]
fn the_embargos_values_reach_the_content_but_not_the_package_metadata() {
    let (bytes, warnings) = render_embargos();
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut archive = archive(bytes);
    let package = entry(&mut archive, "OEBPS/content.opf");
    let nav = entry(&mut archive, "OEBPS/nav.xhtml");
    let content = entry(&mut archive, "OEBPS/chapter-2.xhtml");

    assert!(content.contains("55.444.333"), "the body carries the value");
    assert!(!package.contains("55.444.333"), "{package}");
    assert!(!nav.contains("55.444.333"), "{nav}");
}

#[test]
fn a_frontmatter_identifier_is_used_verbatim() {
    let (bytes, _) = render_example(
        "tecnico",
        "---\ntitle: Livro\nlanguage: pt-BR\nidentifier: urn:isbn:9780000000000\n---\n\n# Um\n\nTexto.\n",
    );
    let mut archive = archive(bytes);
    let package = entry(&mut archive, "OEBPS/content.opf");

    assert!(
        package.contains("<dc:identifier id=\"pub-id\">urn:isbn:9780000000000</dc:identifier>"),
        "{package}"
    );
}

#[test]
fn missing_title_and_language_fall_back() {
    let (bytes, _) = render_example("tecnico", "# Um\n\nTexto.\n");
    let mut archive = archive(bytes);
    let package = entry(&mut archive, "OEBPS/content.opf");

    assert!(package.contains("<dc:title>Um</dc:title>"), "{package}");
    assert!(
        package.contains("<dc:language>und</dc:language>"),
        "{package}"
    );
}

#[test]
fn the_committed_examples_match_a_fresh_render() {
    let (contract, _) = render_example("escritorio", CONTRACT);
    let (report, _) = render_example("tecnico", REPORT);
    let (embargos, _) = render_embargos();

    assert_eq!(contract, CONTRACT_EPUB);
    assert_eq!(embargos, EMBARGOS_EPUB);
    assert_eq!(report, REPORT_EPUB);
}

#[test]
fn a_block_image_keeps_its_frame_and_relative_source() {
    let directory = example_directory("tecnico");
    let document = markdown::parse("![Diagrama](Diagrama.png)\n-width:70%-\n-align:center-\n")
        .expect("the image must parse");
    let assets = load_images(&document, &directory).expect("the image must load");
    let template = pageformer::template::Template::from_yaml(
        "version: \"0.7\"\nformatting:\n  image:\n    alignment: center\n    max_width: 100%\n    background: \"#ffffff\"\n    border:\n      top: 1pt\n      right: 1pt\n      bottom: 1pt\n      left: 1pt\n      color: \"#336699\"\n      padding: 6\n",
    )
    .expect("the template must be valid");
    let rendered = epub::render(&document, &template, &assets).expect("the package");
    let mut archive = archive(rendered.into_bytes());
    let chapter = entry(&mut archive, "OEBPS/chapter-1.xhtml");
    let css = entry(&mut archive, "OEBPS/style.css");

    assert!(
        chapter.contains("<figure class=\"image-block align-center\">"),
        "{chapter}"
    );
    assert!(chapter.contains("src=\"images/image-0.png\""), "{chapter}");
    assert!(css.contains("figure.image-block"), "{css}");
    assert!(css.contains("background-color: #ffffff;"), "{css}");
    assert!(css.contains("border-width: 1.00pt"), "{css}");
}

#[test]
fn a_body_image_is_packaged_and_referenced_relatively() {
    let project = TempDir::new().expect("a temporary project");
    fs::write(
        project.path().join("logo.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR",
    )
    .expect("the image must be written");
    let document = Document::new(
        Metadata::default(),
        vec![Block::Paragraph(vec![Inline::Image {
            destination: Destination::new("logo.png"),
            title: Some("Logo".to_owned()),
            alternative: vec![Inline::Text("Logotipo".to_owned())],
        }])],
    );
    let template = pageformer::template::Template::embedded().expect("the embedded template");
    let assets = load_images(&document, project.path()).expect("the image must load");
    let rendered = epub::render(&document, &template, &assets).expect("the package");
    let mut archive = archive(rendered.into_bytes());
    let chapter = entry(&mut archive, "OEBPS/chapter-1.xhtml");
    let mut image = archive
        .by_name("OEBPS/images/image-0.png")
        .expect("the image must be packaged");
    let mut bytes = Vec::new();
    image
        .read_to_end(&mut bytes)
        .expect("the image must be read");

    assert_eq!(bytes, b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR");
    assert!(chapter.contains("src=\"images/image-0.png\""), "{chapter}");
    assert!(!chapter.contains("data:image"), "{chapter}");
}
