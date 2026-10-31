//! Integration tests for the self-contained English brief example.
//!
//! The example owns its template, letterhead and diagram, so it also acts as a
//! regression fixture for a document with a letterhead that is not the legal one.

use std::path::PathBuf;

use pageformer::confidential::ConfidentialStore;
use pageformer::format::Format;
use pageformer::pipeline::{self, DocumentJob, Source};
use pageformer::template::loader;

const BRIEF: &str = include_str!("../assets/examples/brief/Brief.md");
const BRIEF_TEXT: &str = include_str!("../assets/examples/brief/Brief.txt");
const BRIEF_HTML: &str = include_str!("../assets/examples/brief/Brief.html");
const BRIEF_EPUB: &[u8] = include_bytes!("../assets/examples/brief/Brief.epub");

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn example_directory() -> PathBuf {
    manifest().join("assets").join("examples").join("brief")
}

fn job() -> DocumentJob {
    let directory = example_directory();
    let source = Source::Buffer {
        text: BRIEF.to_owned(),
        path: directory.join("Brief.md"),
    };
    DocumentJob::new(source, directory)
}

fn render(format: Format) -> Vec<u8> {
    pipeline::render(&job(), format, &ConfidentialStore::new())
        .expect("the brief must render")
        .bytes()
        .to_vec()
}

#[test]
fn the_brief_renders_every_format() {
    for format in Format::ALL {
        let bytes = render(format);
        assert!(
            !bytes.is_empty(),
            "the {} output must not be empty",
            format.name()
        );
    }
}

#[test]
fn the_brief_matches_its_text_html_and_epub_goldens() {
    assert_eq!(
        String::from_utf8(render(Format::Txt)).expect("the text must be UTF-8"),
        BRIEF_TEXT
    );
    assert_eq!(
        String::from_utf8(render(Format::Html)).expect("the HTML must be UTF-8"),
        BRIEF_HTML
    );
    assert_eq!(render(Format::Epub), BRIEF_EPUB);
}

#[test]
fn the_brief_is_deterministic_in_every_format() {
    for format in Format::ALL {
        assert_eq!(
            render(format),
            render(format),
            "the {} output must be deterministic",
            format.name()
        );
    }
}

#[test]
fn the_brief_pdf_is_pdf_a() {
    let bytes = render(Format::Pdf);

    assert!(bytes.starts_with(b"%PDF-"), "the output must be a PDF");
    assert!(
        bytes
            .windows(b"pdfaid:part".len())
            .any(|w| w == b"pdfaid:part"),
        "the output has no PDF/A marker"
    );
    assert!(
        bytes
            .windows(b"pdfaid:conformance".len())
            .any(|w| w == b"pdfaid:conformance"),
        "the output has no PDF/A conformance marker"
    );
}

#[test]
fn the_brief_owns_a_letterhead_and_a_running_page_counter() {
    let resolved =
        loader::resolve(&example_directory(), None).expect("the brief template must resolve");
    let template = resolved.template();

    assert_eq!(
        template.page().letterhead().map(|path| path.as_str()),
        Some("Letterhead.png")
    );
    let header = template.page().header().expect("the brief has a header");
    assert_eq!(header.slots().right(), Some("{page} / {pages}"));
    assert_eq!(header.slots().left(), None);
}

#[test]
fn the_brief_packages_are_zip_archives() {
    for format in [Format::Odt, Format::Docx] {
        let bytes = render(format);
        assert!(
            bytes.starts_with(b"PK\x03\x04"),
            "the {} package must be a ZIP",
            format.name()
        );
    }
}

#[test]
fn the_brief_html_is_english_and_self_contained() {
    let html = String::from_utf8(render(Format::Html)).expect("the HTML must be UTF-8");

    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(html.contains("lang=\"en-US\""), "{html}");
    assert!(
        html.contains("background-image: url(\"data:image/png;base64,"),
        "the letterhead must be embedded"
    );
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("http://"), "{html}");
    assert!(!html.contains("https://"), "{html}");
}
