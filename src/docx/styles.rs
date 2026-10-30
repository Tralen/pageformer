//! Generates `word/styles.xml` from the template.

use crate::document::Document;
use crate::metrics;
use crate::template::{
    BlockBorder, Color, FontSlant, Length, ParagraphAlignment, PartialTextStyle, RuleStyle,
    Template, TextFace,
};
use crate::xml;

/// Generates the styles part.
pub(crate) fn styles_xml(document: &Document, template: &Template) -> String {
    let paragraph = template.formatting().paragraph();
    let face = paragraph.face();
    let language = language_attribute(document);
    let line = metrics::line_advance(face.size(), paragraph.leading());
    let after = metrics::paragraph_after(paragraph.spacing(), paragraph.leading());
    let mut output = String::new();
    xml::push(
        &mut output,
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n",
    );
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:docDefaults><w:rPrDefault><w:rPr>{fonts}{color}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>{language}</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{spacing}<w:jc w:val=\"{align}\"/></w:pPr></w:pPrDefault></w:docDefaults>\n",
            fonts = run_fonts(face),
            color = foreground(face),
            size = metrics::half_points(face.size().points()),
            spacing = spacing(line, after),
            align = alignment(paragraph.alignment()),
        ),
    );
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/><w:pPr>{spacing}<w:jc w:val=\"{align}\"/></w:pPr><w:rPr>{fonts}{color}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>{language}</w:rPr></w:style>\n",
            fonts = run_fonts(face),
            color = foreground(face),
            size = metrics::half_points(face.size().points()),
            spacing = spacing(line, after),
            align = alignment(paragraph.alignment()),
        ),
    );

    for (index, heading) in template.formatting().heading().levels().iter().enumerate() {
        let level = index.saturating_add(1);
        let face = heading.face();
        let before = metrics::heading_before(
            heading.spacing_before(),
            paragraph.leading(),
            paragraph.spacing(),
        );
        let after = metrics::heading_after(
            heading
                .rule()
                .and_then(RuleStyle::spacing_after)
                .unwrap_or(heading.spacing_after()),
        );
        let border = heading.rule().map_or_else(String::new, |rule| {
            format!(
                "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"{}\" w:space=\"{}\" w:color=\"{}\"/></w:pBdr>",
                metrics::eighths(rule.thickness().points()),
                metrics::border_space(rule.spacing_before().map_or(0.0, Length::points)),
                rule.color().hex().trim_start_matches('#')
            )
        });
        xml::push_fmt(
            &mut output,
            format_args!(
                "<w:style w:type=\"paragraph\" w:styleId=\"Heading{level}\"><w:name w:val=\"heading {level}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:keepNext/>{page_break}{border}{shading}<w:spacing w:before=\"{before}\" w:after=\"{after}\" w:line=\"{line}\" w:lineRule=\"atLeast\"/><w:outlineLvl w:val=\"{outline}\"/></w:pPr><w:rPr>{fonts}{bold}{italic}{color}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/></w:rPr></w:style>\n",
                page_break = if heading.page_break_before() {
                    "<w:pageBreakBefore/>"
                } else {
                    ""
                },
                border = border,
                shading = heading.background().map_or_else(String::new, |color| {
                    format!(
                        "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                        color.hex().trim_start_matches('#')
                    )
                }),
                before = metrics::twips(before),
                after = metrics::twips(after),
                line = metrics::twips(line),
                outline = index,
                fonts = run_fonts(face),
                color = foreground(face),
                bold = if face.weight().get() >= 600 {
                    "<w:b/>"
                } else {
                    ""
                },
                italic = if face.slant() == FontSlant::Italic {
                    "<w:i/>"
                } else {
                    ""
                },
                size = metrics::half_points(face.size().points()),
            ),
        );
    }

    if let Some(background) = paragraph.background() {
        xml::push_fmt(
            &mut output,
            format_args!(
                "<w:style w:type=\"paragraph\" w:styleId=\"BodyParagraph\"><w:name w:val=\"Body Paragraph\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr>{}</w:pPr></w:style>\n",
                shading(Some(background))
            ),
        );
    }

    let quote = template.formatting().quote();
    let quote_rpr = rpr(partial_properties(Some(quote.text()), None, false));
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Quote\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/><w:pPr>{border}{shading}<w:ind w:left=\"567\" w:right=\"567\"/><w:jc w:val=\"{align}\"/></w:pPr>{quote_rpr}</w:style>\n",
            border = border_xml(quote.border()),
            shading = shading(quote.text().background()),
            align = alignment(quote.alignment())
        ),
    );
    let code_block = template.formatting().code_block();
    let code_rpr = rpr(partial_properties(
        Some(code_block.text()),
        Some("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/>"),
        false,
    ));
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Code\"><w:name w:val=\"Code\"/><w:basedOn w:val=\"Normal\"/><w:pPr>{border}{shading}<w:suppressAutoHyphens/><w:jc w:val=\"{align}\"/></w:pPr>{code_rpr}</w:style>\n",
            border = border_xml(code_block.border()),
            shading = shading(code_block.text().background()),
            align = alignment(code_block.alignment())
        ),
    );
    xml::push(
        &mut output,
        "<w:style w:type=\"paragraph\" w:styleId=\"FootnoteText\"><w:name w:val=\"footnote text\"/><w:basedOn w:val=\"Normal\"/></w:style>\n\
         <w:style w:type=\"character\" w:styleId=\"FootnoteReference\"><w:name w:val=\"footnote reference\"/><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>\n\
         <w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:rPr><w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/></w:rPr></w:style>\n",
    );
    let code_char = partial_rpr(Some(template.formatting().paragraph().code()), None);
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:style w:type=\"character\" w:styleId=\"CodeChar\"><w:name w:val=\"Code Char\"/>{code_char}</w:style>\n"
        ),
    );
    xml::push(&mut output, "</w:styles>\n");
    output
}

/// The paragraph border of the declared sides, with the padding as the border
/// space, which is how OOXML keeps the text away from the rule.
fn border_xml(border: &BlockBorder) -> String {
    if border.is_empty() {
        return String::new();
    }
    let color = border.color().hex();
    let color = color.trim_start_matches('#');
    let space = metrics::border_space(border.padding().points());
    let mut output = String::from("<w:pBdr>");
    for (side, thickness) in [
        ("top", border.top()),
        ("left", border.left()),
        ("bottom", border.bottom()),
        ("right", border.right()),
    ] {
        if let Some(thickness) = thickness {
            output.push_str(&format!(
                "<w:{side} w:val=\"single\" w:sz=\"{}\" w:space=\"{space}\" w:color=\"{color}\"/>",
                metrics::eighths(thickness.points())
            ));
        }
    }
    output.push_str("</w:pBdr>");
    output
}

fn run_fonts(face: &TextFace) -> String {
    let family = match face.font_families().first() {
        Some(family) => family.as_str(),
        None => "Liberation Serif",
    };
    let name = xml::escape_attribute(family);
    format!(
        "<w:rFonts w:ascii=\"{name}\" w:hAnsi=\"{name}\" w:eastAsia=\"{name}\" w:cs=\"{name}\"/>"
    )
}

/// A run property block for a partial style, with the default font kept when the
/// template does not declare families.
fn partial_rpr(style: Option<&PartialTextStyle>, default_fonts: Option<&str>) -> String {
    let properties = partial_properties(style, default_fonts, true);
    if properties.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{properties}</w:rPr>")
    }
}

/// Wraps run properties in `w:rPr` when there is anything to declare.
fn rpr(properties: String) -> String {
    if properties.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{properties}</w:rPr>")
    }
}

/// The run properties of a partial style, without the `w:rPr` wrapper.
pub(crate) fn partial_properties(
    style: Option<&PartialTextStyle>,
    default_fonts: Option<&str>,
    include_shading: bool,
) -> String {
    let mut properties = String::new();
    match style
        .and_then(PartialTextStyle::font_families)
        .and_then(|families| families.first())
    {
        Some(family) => {
            properties.push_str(&run_fonts_named(family.as_str()));
        }
        None => {
            if let Some(default_fonts) = default_fonts {
                properties.push_str(default_fonts);
            }
        }
    }
    if let Some(size) = style.and_then(PartialTextStyle::size) {
        let half = metrics::half_points(size.points());
        properties.push_str(&format!(
            "<w:sz w:val=\"{half}\"/><w:szCs w:val=\"{half}\"/>"
        ));
    }
    if let Some(font_weight) = style.and_then(PartialTextStyle::weight)
        && font_weight.get() >= 600
    {
        properties.push_str("<w:b/>");
    }
    if let Some(slant) = style.and_then(PartialTextStyle::slant)
        && slant == FontSlant::Italic
    {
        properties.push_str("<w:i/>");
    }
    if let Some(color) = style.and_then(PartialTextStyle::foreground) {
        properties.push_str(&format!(
            "<w:color w:val=\"{}\"/>",
            color.hex().trim_start_matches('#')
        ));
    }
    if include_shading && let Some(color) = style.and_then(PartialTextStyle::background) {
        properties.push_str(&shading(Some(color)));
    }
    properties
}

fn run_fonts_named(family: &str) -> String {
    let name = xml::escape_attribute(family);
    format!(
        "<w:rFonts w:ascii=\"{name}\" w:hAnsi=\"{name}\" w:eastAsia=\"{name}\" w:cs=\"{name}\"/>"
    )
}

fn shading(background: Option<Color>) -> String {
    background.map_or_else(String::new, |color| {
        format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            color.hex().trim_start_matches('#')
        )
    })
}

fn foreground(face: &TextFace) -> String {
    face.foreground().map_or_else(String::new, |color| {
        format!(
            "<w:color w:val=\"{}\"/>",
            color.hex().trim_start_matches('#')
        )
    })
}

fn language_attribute(document: &Document) -> String {
    match document.metadata().language() {
        Some(language) => format!(
            "<w:lang w:val=\"{}\"/>",
            xml::escape_attribute(language.as_str())
        ),
        None => String::new(),
    }
}

fn spacing(line: f64, after: f64) -> String {
    format!(
        "<w:spacing w:after=\"{}\" w:line=\"{}\" w:lineRule=\"atLeast\"/>",
        metrics::twips(after),
        metrics::twips(line)
    )
}

fn alignment(alignment: ParagraphAlignment) -> &'static str {
    match alignment {
        ParagraphAlignment::Justified => "both",
        ParagraphAlignment::Left => "left",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Metadata;
    use crate::template::Template;

    #[test]
    fn the_normal_style_carries_the_template_face_and_spacing() {
        let template = Template::from_yaml(
            "version: \"0.5\"\nformatting:\n  paragraph:\n    size: 12\n    leading: 8\n    spacing: 24\n",
        )
        .expect("the template must be valid");
        let styles = styles_xml(&Document::new(Metadata::default(), Vec::new()), &template);

        assert!(styles.contains("<w:sz w:val=\"24\"/>"), "{styles}");
        assert!(styles.contains("w:line=\"318\""), "{styles}");
        assert!(styles.contains("w:after=\"320\""), "{styles}");
    }
}
