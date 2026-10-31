//! Structural and content tests for the HTML backend.

use std::fs;
use std::path::{Path, PathBuf};

use pageformer::assets::load_assets;
use pageformer::confidential::loader as confidential_loader;
use pageformer::confidential::{self, ConfidentialStore};
use pageformer::document::{Block, Destination, Document, Inline, Metadata};
use pageformer::html::{self, HtmlRenderer};
use pageformer::markdown;
use pageformer::render::Renderer;
use pageformer::template::Template;
use pageformer::template::loader;
use tempfile::TempDir;

const CONTRACT: &str = include_str!("../assets/examples/escritorio/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/escritorio/Embargos.md");
const REPORT: &str = include_str!("../assets/examples/tecnico/Relatorio.md");
const EMBARGOS_VALUES: &str = include_str!("../assets/examples/confidencial/embargos.ini");
const CONTRACT_HTML: &str = include_str!("../assets/examples/publicacao/Contrato.html");
const EMBARGOS_HTML: &str = include_str!("../assets/examples/publicacao/Embargos.html");
const REPORT_HTML: &str = include_str!("../assets/examples/tecnico/Relatorio.html");

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

fn parser(text: &str) -> roxmltree::Document<'_> {
    let body = text.strip_prefix("<!DOCTYPE html>\n").unwrap_or(text);
    roxmltree::Document::parse(body).expect("the HTML must be well formed")
}

fn normalized_text(document: &roxmltree::Document<'_>) -> String {
    let mut text = String::new();
    collect_text(document.root(), &mut text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text(node: roxmltree::Node<'_, '_>, text: &mut String) {
    if node.is_element() && node.tag_name().name() == "style" {
        return;
    }
    if node.is_text()
        && let Some(value) = node.text()
    {
        text.push_str(value);
        text.push(' ');
    }
    for child in node.children() {
        collect_text(child, text);
    }
}

fn has_element(document: &roxmltree::Document<'_>, name: &str) -> bool {
    document
        .descendants()
        .any(|node| node.is_element() && node.tag_name().name() == name)
}

fn render_example(directory: &str, source: &str) -> (String, Vec<String>) {
    let directory = example_directory(directory);
    let resolved = loader::resolve(&directory, None).expect("the example template must resolve");
    let document = markdown::parse(source).expect("the fixture must parse");
    let assets = load_assets(
        &document,
        resolved.template(),
        &directory,
        resolved.base_directory(),
    )
    .expect("the example assets must load");
    let rendered = html::render(&document, resolved.template(), &assets);
    let warnings = rendered.warnings().to_vec();
    (
        String::from_utf8(rendered.into_bytes()).expect("the HTML must be UTF-8"),
        warnings,
    )
}

fn render_embargos() -> (String, Vec<String>) {
    let directory = example_directory("escritorio");
    let resolved = loader::resolve(&directory, None).expect("the template must resolve");
    let document = markdown::parse(EMBARGOS).expect("the fixture must parse");
    let resolution = confidential::resolve(&document, &example_store())
        .expect("the example values must resolve the fixture");
    let assets = load_assets(
        resolution.document(),
        resolved.template(),
        &directory,
        resolved.base_directory(),
    )
    .expect("the assets must load");
    let rendered = html::render(resolution.document(), resolved.template(), &assets);
    let warnings = rendered.warnings().to_vec();
    (
        String::from_utf8(rendered.into_bytes()).expect("the HTML must be UTF-8"),
        warnings,
    )
}

#[test]
fn the_contract_is_a_self_contained_semantic_document() {
    let (html, warnings) = render_example("escritorio", CONTRACT);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(html.ends_with("</html>\n"), "{html}");
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("http://"), "{html}");
    assert!(!html.contains("https://"), "{html}");

    let document = parser(&html);
    let root = document.root_element();
    assert_eq!(root.tag_name().name(), "html");
    assert_eq!(root.attribute("lang"), Some("pt-BR"));
    let title = document
        .descendants()
        .find(|node| node.has_tag_name("title"))
        .and_then(|node| node.text());
    assert_eq!(title, Some("Contrato de Prestação de Serviços"));
    assert!(has_element(&document, "main"));
    assert!(has_element(&document, "h2"));
    assert!(has_element(&document, "p"));
    assert!(has_element(&document, "table"));
    assert!(has_element(&document, "thead"));
    assert!(has_element(&document, "blockquote"));
    assert!(
        html.contains("background-image: url(\"data:image/png;base64,"),
        "the letterhead must be embedded in the print page"
    );
    assert!(
        html.contains("main.pageformer"),
        "the screen page must follow the template"
    );
    assert!(normalized_text(&document).contains("Das partes"));
    assert!(normalized_text(&document).contains("Contrato de Prestação de Serviços"));
}

#[test]
fn a_page_background_reaches_the_screen_page_and_the_print_paper() {
    let template = Template::from_yaml("version: \"0.7\"\npage:\n  background: \"#fafafa\"\n")
        .expect("the template must be valid");
    let document = markdown::parse("Texto.\n").expect("the document must parse");
    let rendered = html::render(&document, &template, &pageformer::assets::Assets::default());
    let html = String::from_utf8(rendered.into_bytes()).expect("the HTML must be UTF-8");

    assert!(
        html.contains("background: #fafafa;"),
        "the screen page carries the fill: {html}"
    );
    assert!(
        html.contains("html, body { background: #fafafa; }"),
        "the print paper carries the fill: {html}"
    );
    assert!(
        html.contains("background-color: #fafafa;"),
        "the print page carries the fill: {html}"
    );
}

#[test]
fn the_two_renders_are_byte_identical() {
    let (first, _) = render_example("tecnico", REPORT);
    let (second, _) = render_example("tecnico", REPORT);

    assert_eq!(first, second);
}

#[test]
fn the_report_keeps_its_table_alert_code_and_endnotes() {
    let (html, warnings) = render_example("tecnico", REPORT);
    assert!(warnings.is_empty(), "{warnings:?}");

    let document = parser(&html);
    assert!(has_element(&document, "table"));
    assert!(has_element(&document, "aside"));
    assert!(has_element(&document, "pre"));
    assert!(has_element(&document, "code"));
    assert!(has_element(&document, "section"));
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("section") && node.attribute("role") == Some("doc-endnotes")
        }),
        "the endnotes section must exist"
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("li") && node.attribute("role") == Some("doc-endnote")
        }),
        "the endnote item must exist"
    );
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("a") && node.attribute("role") == Some("doc-backlink")
        }),
        "the backlink must exist"
    );
    assert!(normalized_text(&document).contains("Relatório Técnico"));
    assert!(normalized_text(&document).contains("A nota de rodapé"));
}

#[test]
fn the_embargos_values_reach_the_body_but_not_the_head() {
    let (html, warnings) = render_embargos();
    assert!(warnings.is_empty(), "{warnings:?}");

    assert!(html.contains("55.444.333"), "the body carries the value");
    let document = parser(&html);
    let mut head = String::new();
    if let Some(head_node) = document
        .descendants()
        .find(|node| node.has_tag_name("head"))
    {
        collect_text(head_node, &mut head);
    }
    assert!(
        !head.contains("55.444.333"),
        "the head must not carry a confidential value"
    );
}

#[test]
fn the_committed_examples_match_a_fresh_render() {
    let (contract, _) = render_example("escritorio", CONTRACT);
    let (report, _) = render_example("tecnico", REPORT);
    let (embargos, _) = render_embargos();

    assert_eq!(contract, CONTRACT_HTML);
    assert_eq!(embargos, EMBARGOS_HTML);
    assert_eq!(report, REPORT_HTML);
}

#[test]
fn the_renderer_loads_the_letterhead_and_renders() {
    let directory = example_directory("escritorio");
    let resolved = loader::resolve(&directory, None).expect("the template must resolve");
    let document = markdown::parse(CONTRACT).expect("the fixture must parse");
    let renderer = HtmlRenderer::load(
        &document,
        resolved.template(),
        &directory,
        resolved.base_directory(),
    )
    .expect("the letterhead must load");
    let rendered = renderer
        .render(&document, resolved.template())
        .expect("the document must render");
    let html = String::from_utf8(rendered.into_bytes()).expect("the HTML must be UTF-8");

    assert!(html.contains("data:image/png;base64,"), "{html}");
}

#[test]
fn a_local_image_becomes_a_data_uri() {
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
    let assets =
        load_assets(&document, &template, project.path(), None).expect("the image must load");
    let rendered = html::render(&document, &template, &assets);
    let html = String::from_utf8(rendered.into_bytes()).expect("the HTML must be UTF-8");

    assert!(html.contains("src=\"data:image/png;base64,"), "{html}");
    assert!(html.contains("alt=\"Logotipo\""), "{html}");
    assert!(html.contains("title=\"Logo\""), "{html}");
}
