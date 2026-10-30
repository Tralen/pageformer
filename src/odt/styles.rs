//! Generates `styles.xml`: named styles, the page layout and the master page.

use std::collections::BTreeSet;

use crate::assets::Assets;
use crate::document::Document;
use crate::template::{
    FontSlant, Length, Orientation, ParagraphAlignment, RuleStyle, RunningText, Template, TextFace,
};

use super::picture_name;
use crate::xml;

const PREFIX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" office:version="1.3">
<office:font-face-decls>
"#;

/// Generates the styles part.
pub fn styles_xml(document: &Document, template: &Template, assets: &Assets) -> String {
    let mut output = String::new();
    xml::push(&mut output, PREFIX);
    for family in font_families(template) {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<style:font-face style:name=\"{name}\" svg:font-family=\"'{name}'\" style:font-family-generic=\"roman\" style:font-pitch=\"variable\"/>\n",
                name = xml::escape_attribute(&family)
            ),
        );
    }
    xml::push(&mut output, "</office:font-face-decls>\n<office:styles>\n");

    let paragraph = template.formatting().paragraph();
    let face = paragraph.face();
    let language = language_attributes(document.metadata().language().map(|value| value.as_str()));
    xml::push_fmt(
        &mut output,
        format_args!(
            "<style:default-style style:family=\"paragraph\"><style:paragraph-properties fo:orphans=\"2\" fo:widows=\"2\"/><style:text-properties fo:font-family=\"'{family}'\" fo:font-size=\"{size}\" fo:font-weight=\"{weight}\" fo:font-style=\"{slant}\"{language}/></style:default-style>\n",
            family = xml::escape_attribute(first_family(face)),
            size = xml::points(face.size()),
            weight = weight(face.weight().get()),
            slant = slant(face.slant()),
            language = language,
        ),
    );
    xml::push_fmt(
        &mut output,
        format_args!(
            "<style:style style:name=\"Standard\" style:family=\"paragraph\" style:class=\"text\"><style:paragraph-properties {line_height}{margin_bottom}fo:hyphenate=\"true\" fo:text-align=\"{align}\"/><style:text-properties style:font-name=\"{family}\" fo:font-size=\"{size}\" fo:font-weight=\"{weight}\" fo:font-style=\"{slant}\"{language}/></style:style>\n",
            line_height = line_height_attribute(face.size(), paragraph.leading()),
            margin_bottom = margin_bottom(paragraph.spacing(), paragraph.leading()),
            align = alignment(paragraph.alignment()),
            family = xml::escape_attribute(first_family(face)),
            size = xml::points(face.size()),
            weight = weight(face.weight().get()),
            slant = slant(face.slant()),
            language = language,
        ),
    );

    character_style(&mut output, "Emphasis", paragraph.emphasis());
    character_style(&mut output, "Strong", paragraph.strong());
    xml::push(
        &mut output,
        "<style:style style:name=\"Strikethrough\" style:family=\"text\"><style:text-properties style:text-line-through-style=\"solid\"/></style:style>\n",
    );
    xml::push(
        &mut output,
        "<style:style style:name=\"CodeInline\" style:family=\"text\"><style:text-properties fo:font-family=\"'Liberation Mono', monospace\"/></style:style>\n",
    );

    for (index, heading) in template.formatting().heading().levels().iter().enumerate() {
        let level = index.saturating_add(1);
        let face = heading.face();
        xml::push_fmt(
            &mut output,
            format_args!(
                "<style:style style:name=\"Heading{level}\" style:family=\"paragraph\" style:parent-style-name=\"Standard\" style:next-style-name=\"Standard\"><style:paragraph-properties {line_height}fo:margin-top=\"{before}\" fo:margin-bottom=\"{after}\" fo:keep-with-next=\"always\" fo:hyphenate=\"true\" style:default-outline-level=\"{level}\"{page_break}{background}{rule}/><style:text-properties style:font-name=\"{family}\" fo:font-size=\"{size}\" fo:font-weight=\"{weight}\" fo:font-style=\"{slant}\"/></style:style>\n",
                line_height = line_height_attribute(face.size(), paragraph.leading()),
                background = heading.background().map_or_else(String::new, |color| {
                    format!(" fo:background-color=\"{}\"", color.hex())
                }),
                rule = heading.rule().map_or_else(String::new, |rule| {
                    format!(
                        " fo:padding-bottom=\"{}\" fo:border-bottom=\"{} solid {}\"",
                        xml::points_value(rule.spacing_before().map_or(0.0, Length::points)),
                        xml::points(rule.thickness()),
                        rule.color().hex()
                    )
                }),
                before = xml::points_value(crate::metrics::heading_before(
                    heading.spacing_before(),
                    paragraph.leading(),
                    paragraph.spacing()
                )),
                after = xml::points_value(crate::metrics::heading_after(
                    heading
                        .rule()
                        .and_then(RuleStyle::spacing_after)
                        .unwrap_or(heading.spacing_after())
                )),
                page_break = if heading.page_break_before() {
                    " fo:break-before=\"page\""
                } else {
                    ""
                },
                family = xml::escape_attribute(first_family(face)),
                size = xml::points(face.size()),
                weight = weight(face.weight().get()),
                slant = slant(face.slant()),
            ),
        );
    }

    list_styles(&mut output);
    xml::push(&mut output, "</office:styles>\n<office:automatic-styles>\n");
    page_layout(&mut output, template, assets);
    running_style(&mut output, template, document);
    xml::push(
        &mut output,
        "</office:automatic-styles>\n<office:master-styles>\n",
    );
    master_page(&mut output, document, template);
    xml::push(
        &mut output,
        "</office:master-styles>\n</office:document-styles>\n",
    );
    output
}

fn character_style(output: &mut String, name: &str, face: &TextFace) {
    xml::push_fmt(
        output,
        format_args!(
            "<style:style style:name=\"{name}\" style:family=\"text\"><style:text-properties style:font-name=\"{family}\" fo:font-size=\"{size}\" fo:font-weight=\"{weight}\" fo:font-style=\"{slant}\"/></style:style>\n",
            family = xml::escape_attribute(first_family(face)),
            size = xml::points(face.size()),
            weight = weight(face.weight().get()),
            slant = slant(face.slant()),
        ),
    );
}

fn font_families(template: &Template) -> BTreeSet<String> {
    let paragraph = template.formatting().paragraph();
    let mut families = BTreeSet::new();
    for face in [paragraph.face(), paragraph.emphasis(), paragraph.strong()] {
        if let Some(family) = face.font_families().first() {
            families.insert(family.as_str().to_owned());
        }
    }
    for heading in template.formatting().heading().levels() {
        if let Some(family) = heading.face().font_families().first() {
            families.insert(family.as_str().to_owned());
        }
    }
    families
}

fn first_family(face: &TextFace) -> &str {
    match face.font_families().first() {
        Some(family) => family.as_str(),
        None => "Liberation Serif",
    }
}

fn weight(value: u16) -> &'static str {
    if value >= 600 { "bold" } else { "normal" }
}

fn slant(slant: FontSlant) -> &'static str {
    match slant {
        FontSlant::Normal => "normal",
        FontSlant::Italic => "italic",
    }
}

fn alignment(alignment: ParagraphAlignment) -> &'static str {
    match alignment {
        ParagraphAlignment::Justified => "justify",
        ParagraphAlignment::Left => "left",
    }
}

/// Expands a BCP 47 tag into the ODF language attributes.
fn language_attributes(tag: Option<&str>) -> String {
    let Some(tag) = tag else {
        return String::new();
    };
    let mut parts = tag.split('-');
    let primary = parts.next().unwrap_or_default();
    if primary.is_empty() {
        return String::new();
    }
    match parts.next() {
        Some(region) => format!(
            " fo:language=\"{}\" fo:country=\"{}\"",
            xml::escape_attribute(primary),
            xml::escape_attribute(&region.to_ascii_uppercase()),
        ),
        None => format!(" fo:language=\"{}\"", xml::escape_attribute(primary)),
    }
}

fn margin_bottom(
    spacing: Option<crate::template::Length>,
    leading: crate::template::Length,
) -> String {
    let value = crate::metrics::paragraph_after(spacing, leading);
    if value > 0.0 {
        format!("fo:margin-bottom=\"{}\" ", xml::points_value(value))
    } else {
        String::new()
    }
}

fn line_height_attribute(
    size: crate::template::Length,
    leading: crate::template::Length,
) -> String {
    format!(
        "fo:line-height=\"{}\" ",
        xml::points_value(crate::metrics::line_advance(size, leading))
    )
}

fn list_styles(output: &mut String) {
    for (name, ordered) in [("UnorderedList", false), ("OrderedList", true)] {
        xml::push_fmt(
            output,
            format_args!("<text:list-style style:name=\"{name}\">\n"),
        );
        for level in 1..=10_u32 {
            let indent = format!("{:.2}cm", 0.5 * f64::from(level));
            if ordered {
                xml::push_fmt(
                    output,
                    format_args!(
                        "<text:list-level-style-number text:level=\"{level}\" style:num-format=\"1\" style:num-suffix=\".\"><style:list-level-properties text:space-before=\"{indent}\" text:min-label-width=\"0.6cm\"/></text:list-level-style-number>\n"
                    ),
                );
            } else {
                xml::push_fmt(
                    output,
                    format_args!(
                        "<text:list-level-style-bullet text:level=\"{level}\" text:bullet-char=\"•\"><style:list-level-properties text:space-before=\"{indent}\" text:min-label-width=\"0.5cm\"/></text:list-level-style-bullet>\n"
                    ),
                );
            }
        }
        xml::push(output, "</text:list-style>\n");
    }
}

/// The minimum height reserved for a running header or footer.
const RUNNING_MIN_HEIGHT: f64 = 1.2 * 72.0 / 2.54;
/// The gap between a running header or footer and the text area.
const RUNNING_SPACING: f64 = 0.3 * 72.0 / 2.54;

fn page_layout(output: &mut String, template: &Template, assets: &Assets) {
    let page = template.page();
    let dimensions = page.dimensions();
    let margins = page.margins();
    let orientation = match page.orientation() {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    };
    // Writer adds the header and the footer to the page margins, so the
    // margins are reduced by the reserved band to keep the text area where the
    // template puts it.
    let header_reserve = if page.header().is_some() {
        RUNNING_MIN_HEIGHT + RUNNING_SPACING
    } else {
        0.0
    };
    let footer_reserve = if page.footer().is_some() {
        RUNNING_MIN_HEIGHT + RUNNING_SPACING
    } else {
        0.0
    };
    let top = (margins.top().points() - header_reserve).max(0.0);
    let bottom = (margins.bottom().points() - footer_reserve).max(0.0);
    xml::push(output, "<style:page-layout style:name=\"PageLayout\">\n");
    xml::push_fmt(
        output,
        format_args!(
            "<style:page-layout-properties fo:page-width=\"{width}\" fo:page-height=\"{height}\" style:print-orientation=\"{orientation}\" fo:margin-top=\"{top}\" fo:margin-bottom=\"{bottom}\" fo:margin-left=\"{left}\" fo:margin-right=\"{right}\" style:writing-mode=\"lr-tb\">\n",
            width = xml::centimetres(dimensions.width()),
            height = xml::centimetres(dimensions.height()),
            top = xml::centimetres_points(top),
            bottom = xml::centimetres_points(bottom),
            left = xml::centimetres(margins.left()),
            right = xml::centimetres(margins.right()),
        ),
    );
    if let Some(letterhead) = assets.map().letterhead() {
        xml::push_fmt(
            output,
            format_args!(
                "<style:background-image xlink:href=\"Pictures/{}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\" style:position=\"center\" style:repeat=\"stretch\"/>\n",
                xml::escape_attribute(picture_name(letterhead))
            ),
        );
    }
    xml::push(output, "</style:page-layout-properties>\n");
    if page.header().is_some() {
        xml::push_fmt(
            output,
            format_args!(
                "<style:header-style><style:header-footer-properties fo:min-height=\"{}\" fo:margin-bottom=\"{}\"/></style:header-style>\n",
                xml::centimetres_points(RUNNING_MIN_HEIGHT),
                xml::centimetres_points(RUNNING_SPACING),
            ),
        );
    }
    if page.footer().is_some() {
        xml::push_fmt(
            output,
            format_args!(
                "<style:footer-style><style:header-footer-properties fo:min-height=\"{}\" fo:margin-top=\"{}\"/></style:footer-style>\n",
                xml::centimetres_points(RUNNING_MIN_HEIGHT),
                xml::centimetres_points(RUNNING_SPACING),
            ),
        );
    }
    xml::push(output, "</style:page-layout>\n");
}

fn running_style(output: &mut String, template: &Template, document: &Document) {
    let dimensions = template.page().dimensions();
    let margins = template.page().margins();
    let face = template.formatting().paragraph().face();
    let language = language_attributes(document.metadata().language().map(|value| value.as_str()));
    let text_width =
        (dimensions.width().points() - margins.left().points() - margins.right().points()).max(1.0);
    xml::push_fmt(
        output,
        format_args!(
            "<style:style style:name=\"RunningText\" style:family=\"paragraph\"><style:paragraph-properties fo:text-align=\"left\"><style:tab-stops><style:tab-stop style:position=\"{center}\" style:type=\"center\"/><style:tab-stop style:position=\"{right}\" style:type=\"right\"/></style:tab-stops></style:paragraph-properties><style:text-properties style:font-name=\"{family}\" fo:font-size=\"{size}\" fo:font-weight=\"{weight}\" fo:font-style=\"{slant}\"{language}/></style:style>\n",
            center = xml::centimetres_points(text_width / 2.0),
            right = xml::centimetres_points(text_width),
            family = xml::escape_attribute(first_family(face)),
            size = xml::points(face.size()),
            weight = weight(face.weight().get()),
            slant = slant(face.slant()),
        ),
    );
}

fn master_page(output: &mut String, document: &Document, template: &Template) {
    let page = template.page();
    xml::push(
        output,
        "<style:master-page style:name=\"Standard\" style:page-layout-name=\"PageLayout\">\n",
    );
    if let Some(header) = page.header() {
        xml::push(output, "<style:header>\n");
        running_paragraph(output, header, document);
        xml::push(output, "</style:header>\n");
    }
    if let Some(footer) = page.footer() {
        xml::push(output, "<style:footer>\n");
        running_paragraph(output, footer, document);
        xml::push(output, "</style:footer>\n");
    }
    xml::push(output, "</style:master-page>\n");
}

fn running_paragraph(output: &mut String, running: &RunningText, document: &Document) {
    let slots = running.slots();
    xml::push(output, "<text:p text:style-name=\"RunningText\">");
    slot(output, slots.left(), document);
    xml::push(output, "<text:tab/>");
    slot(output, slots.center(), document);
    xml::push(output, "<text:tab/>");
    slot(output, slots.right(), document);
    xml::push(output, "</text:p>\n");
}

fn slot(output: &mut String, text: Option<&str>, document: &Document) {
    let Some(text) = text else {
        return;
    };
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        xml::push(
            output,
            &xml::escape_text(rest.get(..start).unwrap_or_default()),
        );
        let after = rest.get(start.saturating_add(1)..).unwrap_or_default();
        let Some(end) = after.find('}') else {
            xml::push(
                output,
                &xml::escape_text(rest.get(start..).unwrap_or_default()),
            );
            return;
        };
        let token = after.get(..end).unwrap_or_default();
        match token {
            "page" => xml::push(
                output,
                "<text:page-number text:select-page=\"current\">1</text:page-number>",
            ),
            "pages" => xml::push(output, "<text:page-count>1</text:page-count>"),
            "title" => xml::push(
                output,
                &xml::escape_text(
                    document
                        .metadata()
                        .title()
                        .map(|value| value.as_str())
                        .unwrap_or_default(),
                ),
            ),
            "author" => xml::push(
                output,
                &xml::escape_text(
                    document
                        .metadata()
                        .author()
                        .map(|value| value.as_str())
                        .unwrap_or_default(),
                ),
            ),
            other => {
                xml::push(output, "{");
                xml::push(output, &xml::escape_text(other));
                xml::push(output, "}");
            }
        }
        rest = after.get(end.saturating_add(1)..).unwrap_or_default();
    }
    xml::push(output, &xml::escape_text(rest));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weight_of_at_least_six_hundred_is_bold() {
        assert_eq!(weight(400), "normal");
        assert_eq!(weight(700), "bold");
    }

    #[test]
    fn the_list_styles_define_ten_levels() {
        let mut output = String::new();
        list_styles(&mut output);

        assert_eq!(output.matches("text:level=").count(), 20);
    }
}
