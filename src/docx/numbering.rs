//! Generates `word/numbering.xml`, with one numbering instance per list.

use crate::metrics;
use crate::template::{ListStyle, Template};
use crate::xml;

use super::styles::partial_properties;

/// The marker kind of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumberingKind {
    Bullet,
    Decimal,
}

/// One list in the document, numbered from its own id so a restart is local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListInstance {
    pub kind: NumberingKind,
    pub start: u64,
    pub level: usize,
}

/// The numbering instances collected while the body is built.
#[derive(Debug, Default)]
pub(crate) struct Numbering {
    instances: Vec<ListInstance>,
}

impl Numbering {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an instance and returns its `w:numId`.
    pub fn add(&mut self, kind: NumberingKind, start: u64, level: usize) -> usize {
        self.instances.push(ListInstance { kind, start, level });
        self.instances.len()
    }

    pub fn xml(&self, template: &Template) -> String {
        let mut output = String::new();
        xml::push(
            &mut output,
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n",
        );
        let list = template.formatting().list();
        abstract_levels(&mut output, 0, NumberingKind::Bullet, list);
        abstract_levels(&mut output, 1, NumberingKind::Decimal, list);
        for (index, instance) in self.instances.iter().enumerate() {
            let number = index.saturating_add(1);
            let abstract_id = match instance.kind {
                NumberingKind::Bullet => 0,
                NumberingKind::Decimal => 1,
            };
            xml::push_fmt(
                &mut output,
                format_args!(
                    "<w:num w:numId=\"{number}\"><w:abstractNumId w:val=\"{abstract_id}\"/>"
                ),
            );
            if instance.kind == NumberingKind::Decimal && instance.start != 1 {
                xml::push_fmt(
                    &mut output,
                    format_args!(
                        "<w:lvlOverride w:ilvl=\"{}\"><w:startOverride w:val=\"{}\"/></w:lvlOverride>",
                        instance.level, instance.start
                    ),
                );
            }
            xml::push(&mut output, "</w:num>\n");
        }
        xml::push(&mut output, "</w:numbering>\n");
        output
    }
}

fn abstract_levels(output: &mut String, abstract_id: u8, kind: NumberingKind, list: &ListStyle) {
    xml::push_fmt(
        output,
        format_args!(
            "<w:abstractNum w:abstractNumId=\"{abstract_id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>"
        ),
    );
    let marker_rpr = partial_properties(Some(list.text()), None, false);
    let marker_rpr = if marker_rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{marker_rpr}</w:rPr>")
    };
    for level in 0..9_usize {
        let step = u32::try_from(level.saturating_add(1)).unwrap_or(1);
        let indent = list.indent().points() * f64::from(step);
        let left = metrics::twips(indent + list.marker_gap().points());
        let hanging = metrics::twips(list.marker_gap().points());
        let (format, text) = match kind {
            NumberingKind::Bullet => (
                "bullet".to_owned(),
                xml::escape_attribute(list.bullet(level)),
            ),
            NumberingKind::Decimal => (
                list.numbering_format(level).ooxml_code().to_owned(),
                format!(
                    "%{}{}",
                    level.saturating_add(1),
                    xml::escape_attribute(list.suffix())
                ),
            ),
        };
        xml::push_fmt(
            output,
            format_args!(
                "<w:lvl w:ilvl=\"{level}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{format}\"/><w:lvlText w:val=\"{text}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{left}\" w:hanging=\"{hanging}\"/></w:pPr>{marker_rpr}</w:lvl>"
            ),
        );
    }
    xml::push(output, "</w:abstractNum>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_numbering_has_only_the_abstract_definitions() {
        let xml = Numbering::new().xml(&Template::embedded().expect("the embedded template"));

        assert!(xml.contains("w:abstractNumId=\"0\""));
        assert!(!xml.contains("<w:num "));
    }

    #[test]
    fn every_list_gets_its_own_instance_and_a_start_override() {
        let mut numbering = Numbering::new();
        let first = numbering.add(NumberingKind::Bullet, 1, 0);
        let second = numbering.add(NumberingKind::Decimal, 3, 1);

        assert_eq!((first, second), (1, 2));
        let xml = numbering.xml(&Template::embedded().expect("the embedded template"));
        assert!(xml.contains("<w:num w:numId=\"2\"><w:abstractNumId w:val=\"1\"/>"));
        assert!(xml.contains("<w:lvlOverride w:ilvl=\"1\"><w:startOverride w:val=\"3\"/>"));
    }
}
