//! Structural tests for the ODT backend.

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use pageformer::assets::{Assets, load_images};
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
version: "0.7"
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
    assert!(styles.contains("fo:line-height=\"12.54pt\""), "{styles}");
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
    let source = "Texto[^1].\n\n[^1]: Nota.\n\n-pagebreak-\n\n> [!WARNING]\n> Cuidado.\n";
    let document = markdown::parse(source).expect("the document must parse");
    let bytes = render(&document);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let content = entry(&mut archive, "content.xml");
    let styles = entry(&mut archive, "styles.xml");

    assert!(
        content.contains("<text:note text:note-class=\"footnote\""),
        "{content}"
    );
    assert!(
        content.contains(
            "<text:note-citation text:style-name=\"Footnote_20_anchor\">1</text:note-citation>"
        ),
        "the citation takes the superscript anchor style: {content}"
    );
    assert!(
        styles.contains("style:name=\"Footnote_20_Symbol\"")
            && styles.contains("style:name=\"Footnote_20_anchor\"")
            && styles.contains("style:text-position=\"super 58%\""),
        "the footnote symbol and anchor are superscript: {styles}"
    );
    assert!(content.contains("[!WARNING]"), "{content}");
    assert!(
        content.contains("text:style-name=\"PageBreak\""),
        "{content}"
    );
}

#[test]
fn a_rule_and_a_heading_background_follow_the_template() {
    let document =
        markdown::parse("# Título\n\nTexto.\n\n---\n\nDepois.\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
formatting:
  heading:
    background: "#EEEEEE"
  horizontal_rule:
    thickness: 1.5pt
    color: "#112233"
    spacing_before: 12
"##,
    )
    .expect("the template must be valid");

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");
    let content = entry(&mut archive, "content.xml");

    assert!(
        styles.contains("fo:background-color=\"#eeeeee\""),
        "{styles}"
    );
    assert!(
        content.contains("fo:border-bottom=\"1.50pt solid #112233\""),
        "{content}"
    );
    assert!(content.contains("fo:margin-top=\"12.00pt\""), "{content}");
    assert!(
        content.contains("<text:p text:style-name=\"HorizontalLine\"/>"),
        "{content}"
    );
}

#[test]
fn the_foreground_reaches_the_styles() {
    let document = markdown::parse("# Um\n\nTexto com *ênfase* e **forte**.\n")
        .expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
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

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(
        styles.contains("<style:style style:name=\"Standard\"")
            && styles.contains("fo:font-style=\"normal\" fo:color=\"#112233\""),
        "the body takes the paragraph color: {styles}"
    );
    assert!(
        styles.contains("<style:style style:name=\"Emphasis\" style:family=\"text\">")
            && styles.contains("fo:color=\"#445566\""),
        "emphasis overrides it: {styles}"
    );
    assert!(
        styles.contains("fo:color=\"#778899\""),
        "headings take their own color: {styles}"
    );
    assert_eq!(
        styles.matches("fo:color=\"#112233\"").count(),
        3,
        "the default style, Standard and the running style carry the body color: {styles}"
    );
    assert!(
        !styles.contains("<style:style style:name=\"Strong\" style:family=\"text\"><style:text-properties style:font-name=\"Liberation Serif\" fo:font-size=\"12.00pt\" fo:font-weight=\"700\" fo:font-style=\"normal\" fo:color="),
        "strong inherits instead of repeating the body color: {styles}"
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
version: "0.7"
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

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");
    let content = entry(&mut archive, "content.xml");
    for (name, text) in [("styles.xml", &styles), ("content.xml", &content)] {
        roxmltree::Document::parse(text)
            .unwrap_or_else(|error| panic!("{name} is not well formed: {error}"));
    }

    assert!(
        styles.contains(
            "style:name=\"BodyParagraph\" style:family=\"paragraph\" style:parent-style-name=\"Standard\""
        ) && styles.contains("<style:paragraph-properties fo:background-color=\"#fafafa\"/>"),
        "the body paragraph carries the fill through its own style: {styles}"
    );
    let code_inline = styles
        .split("style:name=\"CodeInline\"")
        .nth(1)
        .expect("the inline code style");
    assert!(code_inline.contains("fo:color=\"#000080\""), "{styles}");
    assert!(
        code_inline.contains("fo:background-color=\"#eeeeee\""),
        "{styles}"
    );
    assert!(
        code_inline.contains("style:font-name=\"DejaVu Sans Mono\""),
        "an inline style changes its own font: {styles}"
    );
    let strike = styles
        .split("style:name=\"Strikethrough\"")
        .nth(1)
        .expect("the strikethrough style");
    assert!(strike.contains("fo:color=\"#dc143c\""), "{styles}");
    assert!(
        content.contains("style:name=\"QuoteParagraph\"")
            && content.contains("fo:background-color=\"#f0f0f0\""),
        "the quote paragraph carries the fill: {content}"
    );
    assert!(
        content.contains("fo:border-left=\"3.00pt solid #cccccc\""),
        "the quote carries its left border: {content}"
    );
    assert!(
        content.contains("fo:padding-left=\"8.00pt\""),
        "the border padding applies to the bordered side: {content}"
    );
    assert!(
        content.contains("style:name=\"QuoteParagraph\"")
            && content.contains("fo:text-align=\"left\""),
        "blocks are left aligned by default: {content}"
    );
    assert!(
        content.contains("style:name=\"Code\"")
            && content.contains("fo:background-color=\"#111111\""),
        "the code block carries the fill: {content}"
    );
}

#[test]
fn lists_take_their_markers_layout_and_text_from_the_template() {
    let document = markdown::parse("- Um\n    - Dois\n\n- Três\n\n1. Primeiro\n    1. Interno\n")
        .expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
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

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");
    let content = entry(&mut archive, "content.xml");

    assert!(
        styles.contains("text:bullet-char=\"*\"") && styles.contains("text:bullet-char=\"+\""),
        "the bullets follow the template: {styles}"
    );
    assert!(
        styles.contains("style:num-format=\"I\" style:num-suffix=\")\""),
        "the first ordered level is upper roman: {styles}"
    );
    assert!(
        styles.contains("style:num-format=\"A\" style:num-suffix=\")\""),
        "the second ordered level is upper alpha: {styles}"
    );
    assert!(
        styles.contains("text:space-before=\"0.600cm\" text:min-label-width=\"0.300cm\""),
        "the first level uses the template indent and gap: {styles}"
    );
    assert!(
        styles.contains("text:space-before=\"1.200cm\" text:min-label-width=\"0.300cm\""),
        "the second level adds one indent step: {styles}"
    );
    assert!(
        content.contains("style:name=\"ListItem\"")
            && content.contains("style:name=\"ListLooseItem\""),
        "tight and loose items get their own styles: {content}"
    );
    assert!(
        content.contains("fo:text-align=\"justify\"")
            && content.contains("fo:background-color=\"#eeeeee\"")
            && content.contains("fo:color=\"#112233\""),
        "the item text follows the template: {content}"
    );
    assert!(
        content.contains("fo:margin-bottom=\"4.20pt\""),
        "the loose spacing is the template spacing minus the leading: {content}"
    );
}

#[test]
fn task_markers_take_their_style_and_can_drop_the_bullet() {
    let document =
        markdown::parse("- [x] Feito\n- [ ] Pendente\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
formatting:
  list:
    task:
      gap: 2
      bullet: false
      done:
        marker: "[X]"
        foreground: "#2e7d32"
        weight: bold
      todo:
        marker: "[ ]"
        foreground: "#777777"
"##,
    )
    .expect("the template must be valid");

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");
    let content = entry(&mut archive, "content.xml");

    assert!(
        styles.contains("<style:style style:name=\"TaskDone\" style:family=\"text\">")
            && styles.contains("fo:color=\"#2e7d32\"")
            && styles.contains("fo:font-weight=\"bold\""),
        "the done marker takes its color and weight: {styles}"
    );
    assert!(
        styles.contains("<style:style style:name=\"TaskTodo\" style:family=\"text\">")
            && styles.contains("fo:color=\"#777777\""),
        "the todo marker takes its color: {styles}"
    );
    assert!(
        styles.contains("style:name=\"TaskList\"")
            && styles.contains("text:bullet-char=\"&#x200B;\""),
        "a task-only list uses the marker-free list style: {styles}"
    );
    assert!(
        content.contains("<text:list text:style-name=\"TaskList\">"),
        "the task-only list drops the bullet: {content}"
    );
    assert!(
        content
            .contains("<text:span text:style-name=\"TaskDone\">[X]</text:span>\u{a0}\u{a0}Feito"),
        "the done marker carries its style and gap: {content}"
    );
    assert!(
        content.contains(
            "<text:span text:style-name=\"TaskTodo\">[ ]</text:span>\u{a0}\u{a0}Pendente"
        ),
        "the todo marker carries its style and gap: {content}"
    );

    let mixed = markdown::parse("- [x] Feito\n- Comum\n").expect("the mixed list must parse");
    let bytes = odt::render(&mixed, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let content = entry(&mut archive, "content.xml");
    assert!(
        content.contains("<text:list text:style-name=\"UnorderedList\">"),
        "a mixed list keeps the bullet: {content}"
    );
}

#[test]
fn links_take_the_template_style() {
    let document =
        markdown::parse("Veja [o site](https://example.org).\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
formatting:
  paragraph:
    link:
      foreground: "#112233"
      underline: true
"##,
    )
    .expect("the template must be valid");

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");
    let content = entry(&mut archive, "content.xml");
    let link = styles
        .split("style:name=\"Link\"")
        .nth(1)
        .expect("the link character style");

    assert!(link.contains("fo:color=\"#112233\""), "{styles}");
    assert!(
        link.contains("style:text-underline-style=\"solid\""),
        "{styles}"
    );
    assert!(
        content.contains("<text:a text:style-name=\"Link\" xlink:type=\"simple\""),
        "{content}"
    );
}

#[test]
fn a_heading_fill_scales_with_the_heading_size() {
    let document =
        markdown::parse("# Um\n\nTexto.\n\n## Dois\n\nTexto.\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
formatting:
  paragraph:
    size: 12
    leading: 8
  heading:
    background: "#eeeeee"
"##,
    )
    .expect("the template must be valid");

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(
        !styles.contains("fo:line-height=\"11.88pt\""),
        "a heading does not use the leading plus cap height: {styles}"
    );
    assert_eq!(
        styles.matches("fo:line-height=\"100%\"").count(),
        6,
        "every heading level uses the font's natural line height: {styles}"
    );
    assert!(
        styles.contains("fo:line-height=\"15.88pt\""),
        "the paragraph keeps the calibrated line height: {styles}"
    );
}

#[test]
fn a_heading_rule_is_a_bottom_border_of_its_level() {
    let document = markdown::parse("## Escopo\n\nTexto.\n").expect("the document must parse");
    let template = Template::from_yaml(
        r##"
version: "0.7"
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

    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(
        styles.contains("fo:padding-bottom=\"3.00pt\" fo:border-bottom=\"1.50pt solid #112233\""),
        "{styles}"
    );
    assert!(styles.contains("fo:margin-bottom=\"14.00pt\""), "{styles}");
    assert_eq!(styles.matches("fo:border-bottom").count(), 1, "{styles}");
}

#[test]
fn the_line_spacing_follows_the_typst_leading_model() {
    let template = Template::from_yaml(
        "version: \"0.7\"\nformatting:\n  paragraph:\n    size: 12\n    leading: 8\n    spacing: 24\n",
    )
    .expect("the template must be valid");
    let document = Document::new(Metadata::default(), Vec::new());
    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(styles.contains("fo:line-height=\"15.88pt\""), "{styles}");
    assert!(styles.contains("fo:margin-bottom=\"16.00pt\""), "{styles}");
}

#[test]
fn table_cells_absorb_the_leading_and_do_not_inherit_paragraph_spacing() {
    let template = Template::from_yaml(
        "version: \"0.7\"\nformatting:\n  paragraph:\n    size: 12\n    leading: 8\n  table:\n    cell_padding: 6pt\n",
    )
    .expect("the template must be valid");
    let document = Document::new(
        Metadata::default(),
        vec![Block::Table(
            Table::new(
                vec![ColumnAlignment::Default],
                vec![TableCell::new(vec![Inline::Text("H".to_owned())])],
                vec![],
            )
            .expect("a table"),
        )],
    );
    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let content = entry(&mut archive, "content.xml");

    assert!(content.contains("fo:padding-top=\"2.00pt\""), "{content}");
    assert!(content.contains("fo:margin-bottom=\"0cm\""), "{content}");
}

#[test]
fn a_block_image_takes_its_size_alignment_and_frame() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("examples")
        .join("tecnico");
    let document = markdown::parse("![Diagrama](Diagrama.png)\n-height:4cm-\n-align:right-\n")
        .expect("the image must parse");
    let assets = load_images(&document, &directory).expect("the image must load");
    let template = Template::from_yaml(
        "version: \"0.7\"\nformatting:\n  image:\n    alignment: center\n    max_width: 100%\n    background: \"#ffffff\"\n    border:\n      top: 1pt\n      right: 1pt\n      bottom: 1pt\n      left: 1pt\n      color: \"#336699\"\n      padding: 6\n",
    )
    .expect("the template must be valid");
    let bytes = odt::render(&document, &template, &assets).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let content = entry(&mut archive, "content.xml");

    assert!(
        content.contains("<text:p text:style-name=\"ImageRight\">"),
        "{content}"
    );
    assert!(content.contains("svg:height=\"4.000cm\""), "{content}");
    assert!(
        content.contains("draw:style-name=\"ImageFrame\""),
        "{content}"
    );
    assert!(content.contains("style:name=\"ImageFrame\""), "{content}");
    assert!(
        content.contains("fo:background-color=\"#ffffff\""),
        "{content}"
    );
    assert!(
        content.contains("fo:border-top=\"1.00pt solid #336699\""),
        "{content}"
    );
}

#[test]
fn a_page_background_becomes_the_page_layout_fill() {
    let template = Template::from_yaml("version: \"0.7\"\npage:\n  background: \"#fafafa\"\n")
        .expect("the template must be valid");
    let document = Document::new(Metadata::default(), Vec::new());
    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(
        styles.contains("fo:background-color=\"#fafafa\""),
        "the page layout carries the fill: {styles}"
    );
}

#[test]
fn a_running_header_reserves_its_band_from_the_page_margin() {
    let template = Template::from_yaml(
        "version: \"0.7\"\npage:\n  margins:\n    top: 30mm\n  header:\n    right: \"{page}\"\n",
    )
    .expect("the template must be valid");
    let document = Document::new(Metadata::default(), Vec::new());
    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(styles.contains("fo:margin-top=\"1.500cm\""), "{styles}");
    assert!(styles.contains("<style:header>"), "{styles}");
}

#[test]
fn the_running_text_uses_the_paragraph_face_and_hyphenates_in_the_document_language() {
    let metadata = pageformer::document::Metadata::new(
        None,
        None,
        Some(pageformer::document::Language::parse("pt-BR").expect("a valid language tag")),
    );
    let template = Template::from_yaml(
        "version: \"0.7\"\nformatting:\n  paragraph:\n    size: 13\npage:\n  header:\n    right: \"{page}\"\n",
    )
    .expect("the template must be valid");
    let document = Document::new(metadata, Vec::new());
    let bytes = odt::render(&document, &template, &Assets::default()).expect("the package");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the package must be a ZIP");
    let styles = entry(&mut archive, "styles.xml");

    assert!(styles.contains("style:name=\"RunningText\""), "{styles}");
    assert!(styles.contains("fo:font-size=\"13.00pt\""), "{styles}");
    assert!(styles.contains("fo:hyphenate=\"true\""), "{styles}");
    assert!(
        styles.contains("fo:language=\"pt\" fo:country=\"BR\""),
        "{styles}"
    );
}

fn minimal_png() -> Vec<u8> {
    // A 1x1 transparent PNG, enough for the dimension reader.
    vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00,
    ]
}
