//! Integration tests for the PDF backend.

use std::fs;
use std::path::Path;

use pageformer::confidential::loader as confidential_loader;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::document::{Block, Destination, Document, Inline, Metadata};
use pageformer::markdown;
use pageformer::pdf::{self, AssetMap, Assets};
use pageformer::template::Template;
use pageformer::template::loader;
use tempfile::TempDir;

const CONTRACT: &str = include_str!("../assets/examples/escritorio/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/escritorio/Embargos.md");
const EMBARGOS_VALUES: &str = include_str!("../assets/examples/confidencial/embargos.env");

fn embedded() -> Template {
    Template::embedded().expect("the embedded template must be valid")
}

fn example_store() -> ConfidentialStore {
    let directory = TempDir::new().expect("a temporary directory");
    let path = directory.path().join("embargos.env");
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

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

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

#[test]
fn the_contract_is_rendered_as_pdf_a_and_stays_deterministic() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let template = embedded();
    let assets = Assets::default();

    let first = pdf::render(&document, &template, &assets).expect("the contract must render");
    assert!(first.bytes().starts_with(b"%PDF-"));
    assert!(
        first.warnings().is_empty(),
        "the contract produced warnings: {:?}",
        first.warnings()
    );
    assert!(
        contains(first.bytes(), b"pdfaid:part"),
        "the output has no PDF/A marker"
    );
    assert!(
        contains(first.bytes(), b"pdfaid:conformance"),
        "the output has no PDF/A conformance marker"
    );

    let second = pdf::render(&document, &template, &assets).expect("the rerender must work");
    assert_eq!(
        first.bytes(),
        second.bytes(),
        "the fixed date must make the output deterministic"
    );
}

#[test]
fn the_embargos_fixture_is_rendered() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");
    assert!(
        !resolution.names().is_empty(),
        "the fixture declares fields"
    );
    let output = pdf::render(resolution.document(), &embedded(), &Assets::default())
        .expect("the embargos fixture must render");
    assert!(output.bytes().starts_with(b"%PDF-"));
    assert!(output.bytes().len() > 10_000);
    assert!(
        output.warnings().is_empty(),
        "the embargos fixture produced warnings: {:?}",
        output.warnings()
    );
}

#[test]
fn the_shared_example_template_uses_its_letterhead() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join("escritorio");
    let resolved = loader::resolve(&examples, None).expect("the example template must resolve");
    assert_eq!(resolved.base_directory(), Some(examples.as_path()));
    let template = resolved.template();
    assert_eq!(
        template.page().letterhead().map(|path| path.as_str()),
        Some("Timbre.png")
    );

    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");
    let assets = pdf::load_assets(
        resolution.document(),
        template,
        &examples,
        resolved.base_directory(),
    )
    .expect("the letterhead must load");
    assert_eq!(assets.map().letterhead(), Some("assets/letterhead.png"));

    let output =
        pdf::render(resolution.document(), template, &assets).expect("the example must render");
    assert!(output.bytes().starts_with(b"%PDF-"));
    assert!(
        output.warnings().is_empty(),
        "the example produced warnings: {:?}",
        output.warnings()
    );
    let text = String::from_utf8_lossy(output.bytes());
    let metadata = text
        .split_once("<x:xmpmeta")
        .and_then(|(_, rest)| rest.split_once("</x:xmpmeta>"))
        .map(|(packet, _)| packet)
        .unwrap_or_default();
    assert!(
        !metadata.contains("55.444.333"),
        "a confidential value reached the XMP metadata"
    );
}

#[test]
fn running_content_tables_and_page_breaks_render() {
    let source = "---\ntitle: Contrato\nauthor: Pageformer\ndate: 2026-03-05\nsubject: Teste\nkeywords:\n  - legal\n---\n\n# Título\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n{pagebreak}\n\nDepois.\n\n# Outra seção\n\nTexto.\n";
    let document = markdown::parse(source).expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.5"
page:
  header:
    left: "{title}"
    right: "Página {page} de {pages}"
  footer:
    center: "{author}"
table:
  border: 1pt
  header_fill: "#eeeeee"
formatting:
  heading:
    h1:
      page_break_before: true
"##,
    )
    .expect("the template must be valid");

    let first =
        pdf::render(&document, &template, &Assets::default()).expect("the document must render");
    assert!(
        first.warnings().is_empty(),
        "the document produced warnings: {:?}",
        first.warnings()
    );
    let second =
        pdf::render(&document, &template, &Assets::default()).expect("the rerender must work");
    assert_eq!(
        first.bytes(),
        second.bytes(),
        "the output must be deterministic"
    );
}

#[test]
fn the_technical_report_uses_its_own_template() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join("tecnico");
    let resolved = loader::resolve(&examples, None).expect("the report template must resolve");
    let template = resolved.template();
    assert_eq!(template.page().letterhead(), None);
    assert!(
        template.page().header().is_some(),
        "the report has a header"
    );
    assert!(
        template.page().footer().is_some(),
        "the report has a footer"
    );

    let document = markdown::parse(include_str!("../assets/examples/tecnico/Relatorio.md"))
        .expect("the report fixture must parse");
    let assets = pdf::load_assets(&document, template, &examples, resolved.base_directory())
        .expect("the report assets must load");
    let first = pdf::render(&document, template, &assets).expect("the report must render");
    assert!(
        first.warnings().is_empty(),
        "the report produced warnings: {:?}",
        first.warnings()
    );
    assert!(contains(first.bytes(), b"pdfaid:part"), "no PDF/A marker");
    let second = pdf::render(&document, template, &assets).expect("the rerender must work");
    assert_eq!(first.bytes(), second.bytes());
}

#[test]
fn every_inline_construct_renders() {
    let project = TempDir::new().expect("a temporary project");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("examples")
            .join("escritorio")
            .join("Timbre.png"),
        project.path().join("logo.png"),
    )
    .expect("the image must be copied");

    let source = "Texto **forte** *ênfase* ~~riscado~~ `código` [link](https://example.org) e uma nota[^1].\n\nAntes ![logo](logo.png) depois.\n\nQuebra dura  \naqui.\n\n[^1]: Nota.";
    let document = markdown::parse(source).expect("the document must parse");
    let assets = pdf::load_assets(&document, &embedded(), project.path(), None)
        .expect("the image must load");
    let output = pdf::render(&document, &embedded(), &assets).expect("the document must render");
    assert!(
        output.warnings().is_empty(),
        "the document produced warnings: {:?}",
        output.warnings()
    );
}

#[test]
fn local_images_are_loaded_and_named() {
    let project = TempDir::new().expect("a temporary project");
    fs::write(project.path().join("logo.png"), b"not a real png")
        .expect("the image must be written");
    let document = document_with_image("logo.png");

    let assets = pdf::load_assets(&document, &embedded(), project.path(), None)
        .expect("the image must load");
    assert_eq!(assets.map().image("logo.png"), Some("assets/image-0.png"));
}

#[test]
fn remote_images_are_rejected() {
    let project = TempDir::new().expect("a temporary project");
    let document = document_with_image("https://example.org/logo.png");
    let error =
        pdf::load_assets(&document, &embedded(), project.path(), None).expect_err("a remote image");
    assert!(
        error
            .to_string()
            .contains("remote images are not supported"),
        "{error}"
    );
}

#[test]
fn user_markup_is_escaped() {
    let document = Document::new(
        Metadata::default(),
        vec![Block::Paragraph(vec![Inline::Text(
            "R$ 5 #x *y* [z]".to_owned(),
        )])],
    );
    let source = pdf::typst_source(&document, &embedded(), &AssetMap::new())
        .expect("the source must be generated");
    assert!(source.contains(r"R\$ 5 \#x \*y\* \[z\]"), "{source}");
}
