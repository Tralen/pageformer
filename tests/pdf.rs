//! Integration tests for the PDF backend.

use std::fs;

use pageformer::document::{Block, Destination, Document, Inline, Metadata};
use pageformer::markdown;
use pageformer::pdf::{self, AssetMap, Assets};
use pageformer::template::Template;
use tempfile::TempDir;

const CONTRACT: &str = include_str!("../assets/examples/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/Embargos.md");

fn embedded() -> Template {
    Template::embedded().expect("the embedded template must be valid")
}

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
    let output = pdf::render(&document, &embedded(), &Assets::default())
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
