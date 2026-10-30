//! Generates `word/styles.xml` from the template.

use crate::document::Document;
use crate::metrics;
use crate::template::{FontSlant, ParagraphAlignment, Template, TextFace};
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
            "<w:docDefaults><w:rPrDefault><w:rPr>{fonts}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>{language}</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{spacing}<w:jc w:val=\"{align}\"/></w:pPr></w:pPrDefault></w:docDefaults>\n",
            fonts = run_fonts(face),
            size = metrics::half_points(face.size().points()),
            spacing = spacing(line, after),
            align = alignment(paragraph.alignment()),
        ),
    );
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/><w:pPr>{spacing}<w:jc w:val=\"{align}\"/></w:pPr><w:rPr>{fonts}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>{language}</w:rPr></w:style>\n",
            fonts = run_fonts(face),
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
        let after = metrics::heading_after(heading.spacing_after());
        xml::push_fmt(
            &mut output,
            format_args!(
                "<w:style w:type=\"paragraph\" w:styleId=\"Heading{level}\"><w:name w:val=\"heading {level}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:keepNext/>{page_break}<w:spacing w:before=\"{before}\" w:after=\"{after}\" w:line=\"{line}\" w:lineRule=\"atLeast\"/><w:outlineLvl w:val=\"{outline}\"/></w:pPr><w:rPr>{fonts}{bold}{italic}<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/></w:rPr></w:style>\n",
                page_break = if heading.page_break_before() {
                    "<w:pageBreakBefore/>"
                } else {
                    ""
                },
                before = metrics::twips(before),
                after = metrics::twips(after),
                line = metrics::twips(line),
                outline = index,
                fonts = run_fonts(face),
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

    xml::push(
        &mut output,
        "<w:style w:type=\"paragraph\" w:styleId=\"Quote\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:ind w:left=\"567\" w:right=\"567\"/></w:pPr></w:style>\n\
         <w:style w:type=\"paragraph\" w:styleId=\"Code\"><w:name w:val=\"Code\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:suppressAutoHyphens/></w:pPr><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/></w:rPr></w:style>\n\
         <w:style w:type=\"paragraph\" w:styleId=\"FootnoteText\"><w:name w:val=\"footnote text\"/><w:basedOn w:val=\"Normal\"/></w:style>\n\
         <w:style w:type=\"character\" w:styleId=\"FootnoteReference\"><w:name w:val=\"footnote reference\"/><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>\n\
         <w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:rPr><w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/></w:rPr></w:style>\n\
         <w:style w:type=\"character\" w:styleId=\"CodeChar\"><w:name w:val=\"Code Char\"/><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/></w:rPr></w:style>\n",
    );
    xml::push(&mut output, "</w:styles>\n");
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
