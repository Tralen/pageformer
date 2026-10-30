//! Tests for the YAML template schema.

use pageformer::template::{
    CURRENT_SCHEMA_VERSION, FontSlant, FontWeight, Length, Orientation, PageSize,
    ParagraphAlignment, SchemaVersion, Template, TemplateError, ValueError,
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

fn value_error_is(error: &TemplateError, field: &str, problem: ValueError) -> bool {
    matches!(
        error,
        TemplateError::Value {
            field: actual,
            problem: actual_problem,
        } if actual.as_str() == field && *actual_problem == problem
    )
}

#[test]
fn a_minimal_template_uses_the_documented_defaults() {
    let template = parse("version: \"0.5\"\n");
    assert_eq!(template.schema_version(), CURRENT_SCHEMA_VERSION);
    assert_eq!(template.page().size(), PageSize::A4);
    assert_eq!(template.page().orientation(), Orientation::Portrait);
    assert_millimetres(millimetres(template.page().margins().top()), 20.0);

    let paragraph = template.formatting().paragraph();
    assert_eq!(paragraph.face().size().points(), 12.0);
    assert_eq!(paragraph.face().weight(), FontWeight::NORMAL);
    assert_eq!(paragraph.face().slant(), FontSlant::Normal);
    assert_eq!(paragraph.face().font_families().len(), 3);
    assert_eq!(paragraph.leading().points(), 7.8);
    assert_eq!(paragraph.spacing(), None);
    assert_eq!(paragraph.alignment(), ParagraphAlignment::Justified);
    assert_eq!(paragraph.emphasis().face().slant(), FontSlant::Italic);
    assert_eq!(paragraph.strong().face().weight(), FontWeight::BOLD);

    let heading = template.formatting().heading().levels();
    assert_eq!(heading[0].face().size().points(), 18.0);
    assert_eq!(heading[0].spacing_before().points(), 18.0);
    assert_eq!(heading[0].spacing_after().points(), 6.0);
    assert_eq!(heading[1].face().size().points(), 14.0);
    assert_eq!(heading[2].face().size().points(), 12.0);
    assert_eq!(heading[0].face().weight(), FontWeight::BOLD);
    assert!(!heading[0].page_break_before());
}

#[test]
fn a_complete_template_is_read_field_by_field() {
    let template = parse(
        r#"
version: "0.5"

page:
  size: letter
  orientation: landscape
  margins:
    top: 30mm
    bottom: 20mm
    left: 30mm
    right: 25mm
  letterhead: images/letterhead.png

formatting:
  paragraph:
    font_families:
      - Liberation Serif
      - DejaVu Serif
    size: 11
    weight: semibold
    style: italic
    leading: 9
    spacing: 6
    alignment: left
    emphasis:
      weight: bold
    strong:
      style: italic
  heading:
    font_families:
      - Liberation Sans
    size: 20
    weight: normal
    style: italic
    spacing_before: 14
    spacing_after: 5
    h1:
      size: 22
      weight: bold
      page_break_before: true
    h2:
      spacing_before: 10
"#,
    );
    assert_eq!(template.page().size(), PageSize::Letter);
    assert_eq!(template.page().orientation(), Orientation::Landscape);
    assert_millimetres(millimetres(template.page().margins().right()), 25.0);
    assert_eq!(
        template.page().letterhead().map(|path| path.as_str()),
        Some("images/letterhead.png")
    );

    let paragraph = template.formatting().paragraph();
    assert_eq!(paragraph.face().size().points(), 11.0);
    assert_eq!(paragraph.face().weight().get(), 600);
    assert_eq!(paragraph.face().slant(), FontSlant::Italic);
    assert_eq!(paragraph.leading().points(), 9.0);
    assert_eq!(paragraph.spacing().map(Length::points), Some(6.0));
    assert_eq!(paragraph.alignment(), ParagraphAlignment::Left);
    assert_eq!(paragraph.face().font_families().len(), 2);
    assert_eq!(paragraph.emphasis().face().weight(), FontWeight::BOLD);
    assert_eq!(paragraph.emphasis().face().slant(), FontSlant::Italic);
    assert_eq!(paragraph.strong().face().weight(), FontWeight::BOLD);
    assert_eq!(paragraph.strong().face().slant(), FontSlant::Italic);

    let heading = template.formatting().heading().levels();
    assert_eq!(heading[0].face().font_families().len(), 1);
    assert_eq!(heading[0].face().size().points(), 22.0);
    assert_eq!(heading[0].face().weight(), FontWeight::BOLD);
    assert_eq!(heading[0].face().slant(), FontSlant::Italic);
    assert_eq!(heading[0].spacing_before().points(), 14.0);
    assert_eq!(heading[0].spacing_after().points(), 5.0);
    assert!(heading[0].page_break_before());

    assert_eq!(heading[1].face().size().points(), 20.0);
    assert_eq!(heading[1].face().weight(), FontWeight::NORMAL);
    assert_eq!(heading[1].spacing_before().points(), 10.0);
    assert_eq!(heading[1].spacing_after().points(), 5.0);
    assert!(!heading[1].page_break_before());

    assert_eq!(heading[2].face().size().points(), 20.0);
    assert_eq!(heading[2].face().weight(), FontWeight::NORMAL);
    assert_eq!(heading[2].spacing_before().points(), 14.0);
}

#[test]
fn a_custom_page_keeps_its_dimensions() {
    let template = parse(
        r#"
version: "0.5"
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
    let template = parse("version: \"0.5\"\npage:\n  size: a4\n  orientation: landscape\n");
    let dimensions = template.page().dimensions();
    assert_millimetres(millimetres(dimensions.width()), 297.0);
    assert_millimetres(millimetres(dimensions.height()), 210.0);
}

#[test]
fn an_unsupported_schema_version_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.4\"\n").expect_err("version 0.4 is not supported");
    assert!(matches!(
        error,
        TemplateError::UnsupportedVersion {
            found,
            supported,
        } if found == SchemaVersion::new(0, 4) && supported == CURRENT_SCHEMA_VERSION
    ));
    assert_eq!(
        error.to_string(),
        "unsupported template schema version 0.4; this build supports version 0.5"
    );
}

#[test]
fn a_missing_version_is_rejected() {
    let error = Template::from_yaml("page:\n  size: a4\n").expect_err("a version is required");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn an_unquoted_version_is_read_as_a_string() {
    let template = parse("version: 0.5\n");
    assert_eq!(template.schema_version(), CURRENT_SCHEMA_VERSION);
}

#[test]
fn the_removed_text_section_is_rejected() {
    let error = Template::from_yaml("version: \"0.5\"\ntext:\n  size: 12\n")
        .expect_err("formatting replaced text");
    let message = error.to_string();
    assert!(message.contains("unknown field `text`"), "{message}");
}

#[test]
fn unknown_keys_are_rejected() {
    for yaml in [
        "version: \"0.5\"\ncolour: red\n",
        "version: \"0.5\"\npage:\n  size: a4\n  bleed: 3mm\n",
        "version: \"0.5\"\nformatting:\n  paragraph:\n    font: Serif\n",
        "version: \"0.5\"\nformatting:\n  heading:\n    h7:\n      size: 12\n",
        "version: \"0.5\"\nformatting:\n  paragraph:\n    emphasis:\n      unknown: 1\n",
        "version: \"0.5\"\npage:\n  header:\n    middle: x\n",
        "version: \"0.5\"\ntable:\n  colour: red\n",
    ] {
        assert!(
            matches!(Template::from_yaml(yaml), Err(TemplateError::Yaml { .. })),
            "`{yaml}` must be rejected"
        );
    }
}

#[test]
fn an_unknown_page_size_lists_the_accepted_names() {
    let error = Template::from_yaml("version: \"0.5\"\npage:\n  size: a3\n")
        .expect_err("a3 is not supported");
    let message = error.to_string();
    assert!(message.contains("a4"), "{message}");
}

#[test]
fn an_unknown_orientation_is_rejected() {
    let error = Template::from_yaml("version: \"0.5\"\npage:\n  orientation: sideways\n")
        .expect_err("sideways is not supported");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn a_length_without_a_unit_is_points() {
    let template = parse("version: \"0.5\"\npage:\n  margins:\n    top: 30\n");
    assert_eq!(template.page().margins().top().points(), 30.0);
}

#[test]
fn an_unknown_length_unit_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    size: 12px\n")
            .expect_err("px is not supported");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.size",
        ValueError::UnknownUnit
    ));
}

#[test]
fn a_zero_font_size_is_rejected() {
    let error = Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    size: 0\n")
        .expect_err("the font size must be positive");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.size",
        ValueError::Zero
    ));
}

#[test]
fn a_zero_heading_size_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  heading:\n    h1:\n      size: 0\n")
            .expect_err("the heading size must be positive");
    assert!(value_error_is(
        &error,
        "formatting.heading.h1.size",
        ValueError::Zero
    ));
}

#[test]
fn a_zero_inline_size_is_rejected() {
    let error = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  paragraph:\n    emphasis:\n      size: 0\n",
    )
    .expect_err("the inline size must be positive");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.emphasis.size",
        ValueError::Zero
    ));
}

#[test]
fn a_negative_leading_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    leading: -7\n")
            .expect_err("the leading must not be negative");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.leading",
        ValueError::Negative
    ));
}

#[test]
fn a_negative_spacing_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    spacing: -1\n")
            .expect_err("the spacing must not be negative");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.spacing",
        ValueError::Negative
    ));
}

#[test]
fn a_negative_heading_spacing_is_rejected() {
    let error = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  heading:\n    h1:\n      spacing_before: -1\n",
    )
    .expect_err("the heading spacing must not be negative");
    assert!(value_error_is(
        &error,
        "formatting.heading.h1.spacing_before",
        ValueError::Negative
    ));
}

#[test]
fn an_empty_font_list_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    font_families: []\n")
            .expect_err("the font list must not be empty");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.font_families",
        ValueError::EmptyList
    ));
}

#[test]
fn a_font_weight_name_and_number_are_accepted() {
    let named = parse("version: \"0.5\"\nformatting:\n  paragraph:\n    weight: semibold\n");
    assert_eq!(named.formatting().paragraph().face().weight().get(), 600);

    let numeric = parse("version: \"0.5\"\nformatting:\n  heading:\n    h2:\n      weight: 650\n");
    assert_eq!(
        numeric.formatting().heading().levels()[1]
            .face()
            .weight()
            .get(),
        650
    );
}

#[test]
fn an_invalid_font_weight_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    weight: 950\n")
            .expect_err("950 is outside the accepted range");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.weight",
        ValueError::InvalidWeight
    ));

    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    weight: heavyish\n")
            .expect_err("the weight name is not supported");
    assert!(value_error_is(
        &error,
        "formatting.paragraph.weight",
        ValueError::InvalidWeight
    ));
}

#[test]
fn an_unknown_style_is_rejected() {
    let error =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  paragraph:\n    style: oblique\n")
            .expect_err("oblique is not supported");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn a_running_header_and_footer_keep_their_slots() {
    let template = parse(
        r#"
version: "0.5"
page:
  header:
    left: "{title}"
    right: "Página {page} de {pages}"
  footer:
    center: "{author}"
    right: ""
"#,
    );
    let header = template.page().header().expect("a header");
    assert_eq!(header.slots().left(), Some("{title}"));
    assert_eq!(header.slots().center(), None);
    assert_eq!(header.slots().right(), Some("Página {page} de {pages}"));
    let footer = template.page().footer().expect("a footer");
    assert_eq!(footer.slots().center(), Some("{author}"));
    assert_eq!(footer.slots().right(), None);
}

#[test]
fn an_empty_running_section_is_none() {
    let template = parse("version: \"0.5\"\npage:\n  header:\n    left: \"  \"\n  footer: {}\n");
    assert_eq!(template.page().header(), None);
    assert_eq!(template.page().footer(), None);
}

#[test]
fn an_unknown_lowercase_token_is_rejected() {
    let error = Template::from_yaml(
        "version: \"0.5\"\npage:\n  header:\n    center: \"Página {pagina}\"\n",
    )
    .expect_err("an unknown token");
    assert!(matches!(
        error,
        TemplateError::UnknownToken { ref field, ref token }
            if field == "page.header.center" && token == "pagina"
    ));
    let message = error.to_string();
    assert!(message.contains("{page}"), "{message}");
}

#[test]
fn a_confidential_field_in_running_content_is_rejected() {
    let error = Template::from_yaml("version: \"0.5\"\npage:\n  header:\n    right: \"{NOME}\"\n")
        .expect_err("confidential fields are not resolved in running content");
    assert!(matches!(
        error,
        TemplateError::ConfidentialToken { ref field, ref token }
            if field == "page.header.right" && token == "NOME"
    ));
    assert!(error.to_string().contains("{NOME}"), "{error}");
}

#[test]
fn the_table_style_defaults_preserve_the_plain_grid() {
    let template = parse("version: \"0.5\"\n");
    let table = template.table();
    assert_eq!(table.border().points(), 0.5);
    assert_eq!(table.border_color().hex(), "#bbbbbb");
    assert_eq!(table.header_fill(), None);
    assert_eq!(table.cell_padding().points(), 6.0);
}

#[test]
fn the_table_style_is_read_field_by_field() {
    let template = parse(
        r##"
version: "0.5"
table:
  border: 1.5pt
  border_color: "#112233"
  header_fill: "#EEEEEE"
  cell_padding: 3mm
"##,
    );
    let table = template.table();
    assert_eq!(table.border().points(), 1.5);
    assert_eq!(table.border_color().hex(), "#112233");
    assert_eq!(
        table.header_fill().map(|color| color.hex()),
        Some("#eeeeee".to_owned())
    );
    assert_millimetres(millimetres(table.cell_padding()), 3.0);
}

#[test]
fn an_invalid_color_is_rejected() {
    for yaml in [
        "version: \"0.5\"\ntable:\n  border_color: bbbbbb\n",
        "version: \"0.5\"\ntable:\n  border_color: \"#bbbb\"\n",
        "version: \"0.5\"\ntable:\n  border_color: \"#gggggg\"\n",
        "version: \"0.5\"\ntable:\n  header_fill: \"notacolor\"\n",
    ] {
        let error = Template::from_yaml(yaml).expect_err("an invalid color");
        assert!(matches!(error, TemplateError::Value { .. }), "{yaml}");
    }
}

#[test]
fn a_heading_page_break_can_be_inherited_or_overridden() {
    let base = parse(
        "version: \"0.5\"\nformatting:\n  heading:\n    page_break_before: true\n    h3:\n      page_break_before: false\n",
    );
    let levels = base.formatting().heading().levels();
    assert!(levels[0].page_break_before());
    assert!(levels[1].page_break_before());
    assert!(!levels[2].page_break_before());
    assert!(levels[5].page_break_before());

    let level = parse(
        "version: \"0.5\"\nformatting:\n  heading:\n    h2:\n      page_break_before: true\n",
    );
    let levels = level.formatting().heading().levels();
    assert!(!levels[0].page_break_before());
    assert!(levels[1].page_break_before());
}

#[test]
fn an_unknown_heading_key_is_rejected_with_page_breaks() {
    let error = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  heading:\n    h1:\n      page_break: true\n",
    )
    .expect_err("the key is not supported");
    assert!(matches!(error, TemplateError::Yaml { .. }));
}

#[test]
fn the_rule_defaults_are_a_black_half_point_line() {
    let template = parse("version: \"0.5\"\n");
    let rule = template.formatting().rule();
    assert_eq!(rule.thickness().points(), 0.5);
    assert_eq!(rule.color().hex(), "#000000");
    assert_eq!(rule.spacing_before(), None);
    assert_eq!(rule.spacing_after(), None);
}

#[test]
fn the_rule_is_read_field_by_field() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  rule:
    thickness: 1.5pt
    color: "#112233"
    spacing_before: 12
    spacing_after: 4mm
"##,
    );
    let rule = template.formatting().rule();
    assert_eq!(rule.thickness().points(), 1.5);
    assert_eq!(rule.color().hex(), "#112233");
    assert_eq!(rule.spacing_before().map(Length::points), Some(12.0));
    assert_millimetres(millimetres(rule.spacing_after().expect("a spacing")), 4.0);
}

#[test]
fn an_invalid_rule_value_is_rejected_with_its_field() {
    let zero = Template::from_yaml("version: \"0.5\"\nformatting:\n  rule:\n    thickness: 0\n")
        .expect_err("a zero thickness");
    assert!(
        value_error_is(&zero, "formatting.rule.thickness", ValueError::Zero),
        "{zero}"
    );

    let color =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  rule:\n    color: notacolor\n")
            .expect_err("an invalid color");
    assert!(
        value_error_is(&color, "formatting.rule.color", ValueError::InvalidColor),
        "{color}"
    );

    let spacing =
        Template::from_yaml("version: \"0.5\"\nformatting:\n  rule:\n    spacing_after: -4\n")
            .expect_err("a negative spacing");
    assert!(
        value_error_is(
            &spacing,
            "formatting.rule.spacing_after",
            ValueError::Negative
        ),
        "{spacing}"
    );

    let unknown = Template::from_yaml("version: \"0.5\"\nformatting:\n  rule:\n    width: 1pt\n")
        .expect_err("the key is not supported");
    assert!(matches!(unknown, TemplateError::Yaml { .. }), "{unknown}");
}

#[test]
fn a_heading_background_is_inherited_and_overridden_per_level() {
    let template = parse(
        "version: \"0.5\"\nformatting:\n  heading:\n    background: \"#eeeeee\"\n    h2:\n      background: \"#DDDDDD\"\n",
    );
    let levels = template.formatting().heading().levels();
    let hex = |index: usize| levels[index].background().map(|color| color.hex());
    assert_eq!(hex(0), Some("#eeeeee".to_owned()));
    assert_eq!(hex(1), Some("#dddddd".to_owned()));
    assert_eq!(hex(5), Some("#eeeeee".to_owned()));

    let plain = parse("version: \"0.5\"\n");
    assert_eq!(plain.formatting().heading().levels()[0].background(), None);
}

#[test]
fn an_invalid_heading_background_names_its_level() {
    let error = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  heading:\n    h3:\n      background: notacolor\n",
    )
    .expect_err("an invalid color");
    assert!(
        value_error_is(
            &error,
            "formatting.heading.h3.background",
            ValueError::InvalidColor
        ),
        "{error}"
    );
}

#[test]
fn a_heading_rule_is_declared_per_level() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  heading:
    h2:
      rule:
        thickness: 1.5pt
        color: "#112233"
        spacing_before: 3
        spacing_after: 9
    h4:
      rule: {}
"##,
    );
    let levels = template.formatting().heading().levels();
    assert!(levels[0].rule().is_none());
    assert!(levels[2].rule().is_none());

    let rule = levels[1].rule().expect("h2 declares a rule");
    assert_eq!(rule.thickness().points(), 1.5);
    assert_eq!(rule.color().hex(), "#112233");
    assert_eq!(rule.spacing_before().map(Length::points), Some(3.0));
    assert_eq!(rule.spacing_after().map(Length::points), Some(9.0));

    let empty = levels[3]
        .rule()
        .expect("an empty h4 rule keeps the defaults");
    assert_eq!(empty.thickness().points(), 0.5);
    assert_eq!(empty.color().hex(), "#000000");
    assert_eq!(empty.spacing_before(), None);
    assert_eq!(empty.spacing_after(), None);
}

#[test]
fn a_base_rule_and_background_are_inherited_by_every_level() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  heading:
    background: "#eeeeee"
    rule:
      thickness: 1pt
      color: "#112233"
    h1:
      size: 20
"##,
    );
    for level in template.formatting().heading().levels() {
        assert_eq!(
            level.background().map(|color| color.hex()),
            Some("#eeeeee".to_owned())
        );
        let rule = level.rule().expect("every level inherits the base rule");
        assert_eq!(rule.thickness().points(), 1.0);
        assert_eq!(rule.color().hex(), "#112233");
    }
}

#[test]
fn a_level_clears_an_inherited_rule_and_background_with_none() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  heading:
    background: "#eeeeee"
    rule:
      thickness: 1pt
    h2:
      rule: none
    h3:
      background: "NONE"
      rule: {}
"##,
    );
    let levels = template.formatting().heading().levels();
    assert!(levels[0].rule().is_some(), "h1 inherits the rule");
    assert!(levels[1].rule().is_none(), "h2 clears the inherited rule");
    assert_eq!(
        levels[1].background().map(|color| color.hex()),
        Some("#eeeeee".to_owned()),
        "a cleared rule leaves the background alone"
    );
    assert!(
        levels[2].background().is_none(),
        "h3 clears the inherited background, in any case"
    );
    let empty = levels[2].rule().expect("an empty map is the default rule");
    assert_eq!(empty.thickness().points(), 0.5);
}

#[test]
fn a_heading_rule_written_as_text_is_rejected() {
    let error = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  heading:\n    h2:\n      rule: \"1pt\"\n",
    )
    .expect_err("a rule is a map or none");
    assert!(error.to_string().contains("a rule is a map"), "{error}");
}

#[test]
fn an_invalid_heading_rule_names_its_level() {
    let zero = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  heading:\n    h3:\n      rule:\n        thickness: 0\n",
    )
    .expect_err("a zero thickness");
    assert!(
        value_error_is(
            &zero,
            "formatting.heading.h3.rule.thickness",
            ValueError::Zero
        ),
        "{zero}"
    );

    let color = Template::from_yaml(
        "version: \"0.5\"\nformatting:\n  heading:\n    h3:\n      rule:\n        color: notacolor\n",
    )
    .expect_err("an invalid color");
    assert!(
        value_error_is(
            &color,
            "formatting.heading.h3.rule.color",
            ValueError::InvalidColor
        ),
        "{color}"
    );
}

#[test]
fn a_foreground_is_read_for_every_text_style() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  paragraph:
    foreground: "#112233"
    emphasis:
      foreground: "#445566"
    strong:
      foreground: "#667788"
  heading:
    foreground: "#778899"
"##,
    );
    let paragraph = template.formatting().paragraph();
    let hex = |face: &pageformer::template::TextFace| face.foreground().map(|color| color.hex());
    assert_eq!(hex(paragraph.face()), Some("#112233".to_owned()));
    assert_eq!(hex(paragraph.emphasis().face()), Some("#445566".to_owned()));
    assert_eq!(hex(paragraph.strong().face()), Some("#667788".to_owned()));
    for level in template.formatting().heading().levels() {
        assert_eq!(hex(level.face()), Some("#778899".to_owned()));
    }

    let plain = parse("version: \"0.5\"\n");
    let paragraph = plain.formatting().paragraph();
    assert_eq!(hex(paragraph.face()), None);
    assert_eq!(hex(paragraph.strong().face()), None);
    assert_eq!(hex(plain.formatting().heading().levels()[0].face()), None);

    let inherited =
        parse("version: \"0.5\"\nformatting:\n  paragraph:\n    foreground: \"#112233\"\n");
    assert_eq!(
        hex(inherited.formatting().paragraph().strong().face()),
        None,
        "an inline span inherits the color instead of copying it, so it keeps a heading color"
    );
}

#[test]
fn a_heading_foreground_can_be_cleared_per_level_with_none() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  heading:
    foreground: "#778899"
    h2:
      foreground: none
"##,
    );
    let levels = template.formatting().heading().levels();
    assert_eq!(
        levels[0].face().foreground().map(|color| color.hex()),
        Some("#778899".to_owned())
    );
    assert_eq!(levels[1].face().foreground(), None);
}

#[test]
fn an_invalid_foreground_names_its_field() {
    for (yaml, field) in [
        (
            "version: \"0.5\"\nformatting:\n  paragraph:\n    foreground: notacolor\n",
            "formatting.paragraph.foreground",
        ),
        (
            "version: \"0.5\"\nformatting:\n  paragraph:\n    strong:\n      foreground: notacolor\n",
            "formatting.paragraph.strong.foreground",
        ),
        (
            "version: \"0.5\"\nformatting:\n  heading:\n    h2:\n      foreground: notacolor\n",
            "formatting.heading.h2.foreground",
        ),
    ] {
        let error = Template::from_yaml(yaml).expect_err("an invalid color");
        assert!(
            value_error_is(&error, field, ValueError::InvalidColor),
            "{field}: {error}"
        );
    }
}

#[test]
fn the_paragraph_and_block_styles_are_read() {
    let template = parse(
        r##"
version: "0.5"
formatting:
  paragraph:
    background: "#fafafa"
    code:
      font_families:
        - DejaVu Sans Mono
      size: 10
      weight: semibold
      style: italic
      foreground: navy
      background: "#eeeeee"
    strikethrough:
      foreground: crimson
  quote:
    foreground: "#334455"
    background: "#f0f0f0"
  code_block:
    font_families:
      - Liberation Mono
    size: 9
    background: "#111111"
"##,
    );
    let paragraph = template.formatting().paragraph();
    assert_eq!(
        paragraph.background().map(|color| color.hex()),
        Some("#fafafa".to_owned())
    );

    let code = paragraph.code().expect("inline code");
    assert_eq!(
        code.font_families()
            .and_then(|families| families.first())
            .map(|family| family.as_str()),
        Some("DejaVu Sans Mono")
    );
    assert_eq!(code.size().map(Length::points), Some(10.0));
    assert_eq!(code.weight().map(|weight| weight.get()), Some(600));
    assert_eq!(code.slant(), Some(FontSlant::Italic));
    assert_eq!(
        code.foreground().map(|color| color.hex()),
        Some("#000080".to_owned())
    );
    assert_eq!(
        code.background().map(|color| color.hex()),
        Some("#eeeeee".to_owned())
    );

    let strike = paragraph.strikethrough().expect("strikethrough");
    assert_eq!(
        strike.foreground().map(|color| color.hex()),
        Some("#dc143c".to_owned())
    );
    assert_eq!(strike.size(), None, "an undeclared key changes nothing");

    let quote = template.formatting().quote();
    assert_eq!(
        quote.text().background().map(|color| color.hex()),
        Some("#f0f0f0".to_owned())
    );
    assert_eq!(quote.alignment(), ParagraphAlignment::Left, "the default");
    assert!(quote.border().is_empty());
    let block = template.formatting().code_block();
    assert_eq!(block.text().size().map(Length::points), Some(9.0));
    assert_eq!(
        block.text().background().map(|color| color.hex()),
        Some("#111111".to_owned())
    );
}

#[test]
fn the_new_styles_are_absent_by_default() {
    let template = parse("version: \"0.5\"\n");
    let paragraph = template.formatting().paragraph();
    assert_eq!(paragraph.background(), None);
    assert!(paragraph.code().is_none());
    assert!(paragraph.strikethrough().is_none());
    for block in [
        template.formatting().quote(),
        template.formatting().code_block(),
    ] {
        assert_eq!(block.alignment(), ParagraphAlignment::Left);
        assert!(block.border().is_empty());
        assert_eq!(block.text().foreground(), None);
        assert_eq!(block.text().background(), None);
    }
    assert_eq!(paragraph.emphasis().background(), None);
}

#[test]
fn an_invalid_color_in_a_new_style_names_its_field() {
    for (yaml, field) in [
        (
            "version: \"0.5\"\nformatting:\n  paragraph:\n    background: notacolor\n",
            "formatting.paragraph.background",
        ),
        (
            "version: \"0.5\"\nformatting:\n  paragraph:\n    code:\n      foreground: notacolor\n",
            "formatting.paragraph.code.foreground",
        ),
        (
            "version: \"0.5\"\nformatting:\n  quote:\n    background: notacolor\n",
            "formatting.quote.background",
        ),
        (
            "version: \"0.5\"\nformatting:\n  code_block:\n    foreground: notacolor\n",
            "formatting.code_block.foreground",
        ),
    ] {
        let error = Template::from_yaml(yaml).expect_err("an invalid color");
        assert!(
            value_error_is(&error, field, ValueError::InvalidColor),
            "{field}: {error}"
        );
    }
}

#[test]
fn an_unknown_key_in_a_new_style_is_rejected() {
    for yaml in [
        "version: \"0.5\"\nformatting:\n  quote:\n    align: left\n",
        "version: \"0.5\"\nformatting:\n  paragraph:\n    code:\n      colors: red\n",
    ] {
        let error = Template::from_yaml(yaml).expect_err("an unknown key");
        assert!(matches!(error, TemplateError::Yaml { .. }), "{error}");
    }
}

#[test]
fn the_embedded_template_matches_the_project_defaults() {
    let template = Template::embedded().expect("the embedded template must be valid");
    assert_eq!(template.schema_version(), CURRENT_SCHEMA_VERSION);
    assert_eq!(template.page().size(), PageSize::A4);
    assert_eq!(template.page().orientation(), Orientation::Portrait);
    assert_eq!(template.page().margins().left().points(), 71.0);

    let paragraph = template.formatting().paragraph();
    assert_eq!(paragraph.face().font_families().len(), 3);
    assert_eq!(paragraph.face().size().points(), 12.0);
    assert_eq!(paragraph.leading().points(), 7.8);

    let heading = template.formatting().heading().levels();
    assert_eq!(heading[0].face().size().points(), 18.0);
    assert_eq!(heading[1].face().size().points(), 14.0);
    assert_eq!(heading[0].face().weight(), FontWeight::BOLD);
}
