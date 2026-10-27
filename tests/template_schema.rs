//! Tests for the YAML template schema.

use pageformer::template::{
    CURRENT_SCHEMA_VERSION, Length, Orientation, PageSize, ParagraphAlignment, Template,
    TemplateError, ValueError,
};

fn parse(yaml: &str) -> Template {
    Template::from_yaml(yaml).expect("the template must be valid")
}

fn millimetres(length: Length) -> f64 {
    length.millimetres()
}

fn assert_millimetres(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{actual} is not {expected} millimetres"
    );
}

#[test]
fn a_minimal_template_uses_the_documented_defaults() {
    let template = parse("version: 4\n");
    assert_eq!(template.schema_version().get(), CURRENT_SCHEMA_VERSION);
    assert_eq!(template.page().size(), PageSize::A4);
    assert_eq!(template.page().orientation(), Orientation::Portrait);
    assert_millimetres(millimetres(template.page().margins().top()), 20.0);
    assert_eq!(template.text().body().size().points(), 12.0);
    assert_eq!(template.text().title().size().points(), 18.0);
    assert_eq!(template.text().leading().points(), 7.8);
    assert_eq!(template.text().alignment(), ParagraphAlignment::Justified);
    assert_eq!(template.text().body().font_families().len(), 3);
    assert_eq!(template.text().title().font_families().len(), 3);
    assert_eq!(template.text().paragraph_spacing(), None);
    assert_eq!(template.text().header_spacing(), None);
}

#[test]
fn a_complete_template_is_read_field_by_field() {
    let template = parse(
        r#"
version: 4

page:
  size: letter
  orientation: landscape
  margins:
    top: 30mm
    bottom: 20mm
    left: 30mm
    right: 25mm
  letterhead: images/letterhead.png

text:
  body:
    font_families:
      - Liberation Serif
      - DejaVu Serif
    size: 11
  title:
    font_families:
      - Liberation Sans
    size: 16
  leading: 7.5
  paragraph_spacing: 6
  header_spacing: 18
  alignment: left
"#,
    );
    assert_eq!(template.page().size(), PageSize::Letter);
    assert_eq!(template.page().orientation(), Orientation::Landscape);
    assert_millimetres(millimetres(template.page().margins().right()), 25.0);
    assert_eq!(
        template.page().letterhead().map(|path| path.as_str()),
        Some("images/letterhead.png")
    );
    assert_eq!(template.text().body().size().points(), 11.0);
    assert_eq!(template.text().title().size().points(), 16.0);
    assert_eq!(template.text().leading().points(), 7.5);
    assert_eq!(
        template.text().paragraph_spacing().map(Length::points),
        Some(6.0)
    );
    assert_eq!(
        template.text().header_spacing().map(Length::points),
        Some(18.0)
    );
    assert_eq!(template.text().alignment(), ParagraphAlignment::Left);
    assert_eq!(template.text().body().font_families().len(), 2);
    assert_eq!(template.text().title().font_families().len(), 1);
}

#[test]
fn a_custom_page_keeps_its_dimensions() {
    let template = parse(
        r#"
version: 4
page:
  size:
    width: 216mm
    height: 330mm
"#,
    );
    let dimensions = template.page().dimensions();
    assert_millimetres(millimetres(dimensions.width()), 216.0);
    assert_millimetres(millimetres(dimensions.height()), 330.0);
    assert!(matches!(template.page().size(), PageSize::Custom(_)));
}

#[test]
fn landscape_swaps_the_page_dimensions() {
    let template = parse("version: 4\npage:\n  size: a4\n  orientation: landscape\n");
    let dimensions = template.page().dimensions();
    assert_millimetres(millimetres(dimensions.width()), 297.0);
    assert_millimetres(millimetres(dimensions.height()), 210.0);
}

#[test]
fn an_unsupported_schema_version_is_rejected() {
    let error = Template::from_yaml("version: 2\n").expect_err("version 2 is not supported");
    assert!(matches!(
        error,
        TemplateError::UnsupportedVersion {
            found: 2,
            supported: CURRENT_SCHEMA_VERSION,
        }
    ));
    assert_eq!(
        error.to_string(),
        "unsupported template schema version 2; this build supports version 4"
    );
}

#[test]
fn a_missing_version_is_rejected() {
    let error = Template::from_yaml("page:\n  size: a4\n").expect_err("a version is required");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn the_removed_document_section_is_rejected() {
    let error = Template::from_yaml("version: 4\ndocument:\n  title: Contrato\n")
        .expect_err("document metadata belongs to the frontmatter");
    let message = error.to_string();
    assert!(message.contains("unknown field `document`"), "{message}");
}

#[test]
fn unknown_keys_are_rejected() {
    for yaml in [
        "version: 4\ncolour: red\n",
        "version: 4\npage:\n  size: a4\n  bleed: 3mm\n",
        "version: 4\ntext:\n  font: Serif\n",
        "version: 4\ntext:\n  body:\n    weight: bold\n",
    ] {
        assert!(
            matches!(Template::from_yaml(yaml), Err(TemplateError::Yaml { .. })),
            "`{yaml}` must be rejected"
        );
    }
}

#[test]
fn an_unknown_page_size_lists_the_accepted_names() {
    let error =
        Template::from_yaml("version: 4\npage:\n  size: a3\n").expect_err("a3 is not supported");
    let message = error.to_string();
    assert!(message.contains("a4"), "{message}");
}

#[test]
fn an_unknown_orientation_is_rejected() {
    let error = Template::from_yaml("version: 4\npage:\n  orientation: sideways\n")
        .expect_err("sideways is not supported");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn a_length_without_a_unit_is_points() {
    let template = parse("version: 4\npage:\n  margins:\n    top: 30\n");
    assert_eq!(template.page().margins().top().points(), 30.0);
}

#[test]
fn an_unknown_length_unit_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  body:\n    size: 12px\n")
        .expect_err("px is not supported");
    assert!(matches!(
        error,
        TemplateError::Value {
            field: "text.body.size",
            problem: ValueError::UnknownUnit,
        }
    ));
}

#[test]
fn a_zero_font_size_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  body:\n    size: 0pt\n")
        .expect_err("the font size must be positive");
    assert!(matches!(
        error,
        TemplateError::Value {
            field: "text.body.size",
            problem: ValueError::Zero,
        }
    ));
}

#[test]
fn a_negative_leading_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  leading: -7\n")
        .expect_err("the leading must not be negative");
    assert!(matches!(
        error,
        TemplateError::Value {
            field: "text.leading",
            problem: ValueError::Negative,
        }
    ));
}

#[test]
fn an_unknown_leading_unit_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  leading: 7px\n")
        .expect_err("px is not supported");
    assert!(matches!(
        error,
        TemplateError::Value {
            field: "text.leading",
            problem: ValueError::UnknownUnit,
        }
    ));
}

#[test]
fn a_negative_header_spacing_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  header_spacing: -1\n")
        .expect_err("the header spacing must not be negative");
    assert!(matches!(
        error,
        TemplateError::Value {
            field: "text.header_spacing",
            problem: ValueError::Negative,
        }
    ));
}

#[test]
fn an_empty_font_list_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  body:\n    font_families: []\n")
        .expect_err("the font list must not be empty");
    assert!(matches!(
        error,
        TemplateError::Value {
            field: "text.body.font_families",
            problem: ValueError::EmptyList,
        }
    ));
}

#[test]
fn an_unknown_text_block_key_is_rejected() {
    let error = Template::from_yaml("version: 4\ntext:\n  body:\n    family: Serif\n")
        .expect_err("the key is unknown");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn the_embedded_template_matches_the_project_defaults() {
    let template = Template::embedded().expect("the embedded template must be valid");
    assert_eq!(template.schema_version().get(), CURRENT_SCHEMA_VERSION);
    assert_eq!(template.page().size(), PageSize::A4);
    assert_eq!(template.page().orientation(), Orientation::Portrait);
    assert_eq!(template.page().margins().left().points(), 71.0);
    assert_eq!(template.text().body().font_families().len(), 3);
    assert_eq!(template.text().body().size().points(), 12.0);
    assert_eq!(template.text().title().size().points(), 18.0);
    assert_eq!(template.text().leading().points(), 7.8);
    assert_eq!(template.text().header_spacing(), None);
}
