//! Generates `content.xml`: the document body and the automatic styles it uses.

use std::collections::BTreeMap;

use crate::assets::Assets;
use crate::document::{
    Alert, Block, CodeBlock, ColumnAlignment, Destination, Document, FootnoteDefinition,
    FootnoteId, Inline, List, ListItem, ListStart, Quote, QuoteKind, Table, TableCell, TaskState,
};
use crate::template::Template;

use super::picture_name;
use super::styles::{background_attribute, partial_properties};
use crate::template::PartialTextStyle;
use crate::xml;

const PREFIX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" office:version="1.3">
<office:automatic-styles>
<style:style style:name="PageBreak" style:family="paragraph"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="StandardBreak" style:family="paragraph" style:parent-style-name="Standard"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="Heading1Break" style:family="paragraph" style:parent-style-name="Heading1"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="Heading2Break" style:family="paragraph" style:parent-style-name="Heading2"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="Heading3Break" style:family="paragraph" style:parent-style-name="Heading3"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="Heading4Break" style:family="paragraph" style:parent-style-name="Heading4"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="Heading5Break" style:family="paragraph" style:parent-style-name="Heading5"><style:paragraph-properties fo:break-before="page"/></style:style>
<style:style style:name="Heading6Break" style:family="paragraph" style:parent-style-name="Heading6"><style:paragraph-properties fo:break-before="page"/></style:style>

<style:style style:name="ListItem" style:family="paragraph" style:parent-style-name="Standard"><style:paragraph-properties fo:margin-bottom="0cm"/></style:style>
"#;

/// Generates the content part.
pub fn content_xml(document: &Document, template: &Template, assets: &Assets) -> String {
    let mut builder = Builder::new(template, assets);
    builder.collect_definitions(document.blocks());
    builder.blocks(document.blocks());

    let mut output = String::new();
    xml::push(&mut output, PREFIX);
    block_styles(&mut output, template);
    cell_paragraph_styles(&mut output, template);
    horizontal_line_style(&mut output, template);
    xml::push(&mut output, &builder.generated);
    xml::push(
        &mut output,
        "</office:automatic-styles>\n<office:body><office:text>\n",
    );
    xml::push(&mut output, &builder.body);
    xml::push(
        &mut output,
        "</office:text></office:body></office:document-content>\n",
    );
    output
}

struct Builder<'a> {
    body: String,
    generated: String,
    template: &'a Template,
    assets: &'a Assets,
    definitions: BTreeMap<String, FootnoteDefinition>,
    active: Vec<String>,
    note_count: usize,
    image_count: usize,
    table_count: usize,
    text_width: f64,
    pending_break: bool,
}

impl<'a> Builder<'a> {
    fn new(template: &'a Template, assets: &'a Assets) -> Self {
        let page = template.page();
        let dimensions = page.dimensions();
        let margins = page.margins();
        let text_width =
            (dimensions.width().points() - margins.left().points() - margins.right().points())
                .max(1.0);
        Self {
            body: String::new(),
            generated: String::new(),
            template,
            assets,
            definitions: BTreeMap::new(),
            active: Vec::new(),
            note_count: 0,
            image_count: 0,
            table_count: 0,
            text_width,
            pending_break: false,
        }
    }

    fn collect_definitions(&mut self, blocks: &[Block]) {
        for block in blocks {
            match block {
                Block::FootnoteDefinition(definition) => {
                    self.definitions
                        .insert(definition.id().as_str().to_owned(), definition.clone());
                    self.collect_definitions(definition.blocks());
                }
                Block::List(list) => {
                    for item in list.items() {
                        self.collect_definitions(item.blocks());
                    }
                }
                Block::Quote(quote) => self.collect_definitions(quote.blocks()),
                _ => {}
            }
        }
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.block(block);
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Heading(heading) => {
                let level = heading.level().get();
                let style = self.style_with_break(&format!("Heading{level}"));
                xml::push_fmt(
                    &mut self.body,
                    format_args!(
                        "<text:h text:style-name=\"{style}\" text:outline-level=\"{level}\">"
                    ),
                );
                self.inlines(heading.content());
                xml::push(&mut self.body, "</text:h>\n");
            }
            Block::Paragraph(inlines) => self.paragraph(inlines),
            Block::NumberedParagraph(paragraph) => {
                let style = self.style_with_break(self.body_style());
                self.open_paragraph(&style);
                self.text(&format!("{}. ", paragraph.number()));
                self.inlines(paragraph.content());
                self.close_paragraph();
            }
            Block::List(list) => self.list(list),
            Block::Table(table) => self.table(table),
            Block::Quote(quote) => self.quote(quote),
            Block::CodeBlock(code) => self.code(code),
            Block::ThematicBreak => {
                self.flush_break_as_paragraph();
                xml::push(
                    &mut self.body,
                    "<text:p text:style-name=\"HorizontalLine\"/>\n",
                );
            }
            Block::PageBreak => self.pending_break = true,
            Block::FootnoteDefinition(_) => {}
        }
    }

    fn style_with_break(&mut self, base: &str) -> String {
        if std::mem::take(&mut self.pending_break) {
            format!("{base}Break")
        } else {
            base.to_owned()
        }
    }

    fn flush_break_as_paragraph(&mut self) {
        if std::mem::take(&mut self.pending_break) {
            xml::push(&mut self.body, "<text:p text:style-name=\"PageBreak\"/>\n");
        }
    }

    fn paragraph(&mut self, inlines: &[Inline]) {
        let style = self.style_with_break(self.body_style());
        self.paragraph_with(&style, inlines);
    }

    /// Body paragraphs take the fill through their own style, so quotes, lists and
    /// cells keep the background their own style declares.
    fn body_style(&self) -> &'static str {
        if self
            .template
            .formatting()
            .paragraph()
            .background()
            .is_some()
        {
            "BodyParagraph"
        } else {
            "Standard"
        }
    }

    fn paragraph_with(&mut self, style: &str, inlines: &[Inline]) {
        self.open_paragraph(style);
        self.inlines(inlines);
        self.close_paragraph();
    }

    fn open_paragraph(&mut self, style: &str) {
        xml::push_fmt(
            &mut self.body,
            format_args!(
                "<text:p text:style-name=\"{}\">",
                xml::escape_attribute(style)
            ),
        );
    }

    fn close_paragraph(&mut self) {
        xml::push(&mut self.body, "</text:p>\n");
    }

    fn list(&mut self, list: &List) {
        self.flush_break_as_paragraph();
        let ordered = matches!(list.start(), ListStart::Ordered(_));
        let style = if ordered {
            "OrderedList"
        } else {
            "UnorderedList"
        };
        xml::push_fmt(
            &mut self.body,
            format_args!("<text:list text:style-name=\"{style}\">\n"),
        );
        let mut number = match list.start() {
            ListStart::Ordered(start) => start,
            ListStart::Unordered => 1,
        };
        for (index, item) in list.items().iter().enumerate() {
            if ordered && index == 0 && number != 1 {
                xml::push_fmt(
                    &mut self.body,
                    format_args!("<text:list-item text:start-value=\"{number}\">\n"),
                );
            } else {
                xml::push(&mut self.body, "<text:list-item>\n");
            }
            let last = index.saturating_add(1) == list.items().len();
            self.item(item, list.is_tight() && !last);
            xml::push(&mut self.body, "</text:list-item>\n");
            number = number.saturating_add(1);
        }
        xml::push(&mut self.body, "</text:list>\n");
    }

    fn item(&mut self, item: &ListItem, tight: bool) {
        let base = if tight { "ListItem" } else { "Standard" };
        let marker = match item.task() {
            Some(TaskState::Done) => "[x] ",
            Some(TaskState::Todo) => "[ ] ",
            None => "",
        };
        match item.blocks().first() {
            Some(Block::Paragraph(inlines)) => {
                let style = if self.pending_break {
                    self.style_with_break("Standard")
                } else {
                    base.to_owned()
                };
                self.open_paragraph(&style);
                self.text(marker);
                self.inlines(inlines);
                self.close_paragraph();
                if let Some(rest) = item.blocks().get(1..) {
                    self.blocks(rest);
                }
            }
            _ => {
                if !marker.is_empty() {
                    let style = if self.pending_break {
                        self.style_with_break("Standard")
                    } else {
                        base.to_owned()
                    };
                    self.paragraph_with(&style, &[Inline::Text(marker.trim_end().to_owned())]);
                }
                self.blocks(item.blocks());
            }
        }
    }

    fn table(&mut self, table: &Table) {
        self.flush_break_as_paragraph();
        self.table_count += 1;
        let name = format!("Table{}", self.table_count);
        let columns = table.alignments().len().max(1);
        let column_width = self.text_width / columns as f64;
        let style = self.template.table();
        let border = xml::points(style.border());
        let color = style.border_color().hex();
        let paragraph = self.template.formatting().paragraph();
        let horizontal = style.cell_padding().points();
        let vertical =
            (style.cell_padding().points() - paragraph.leading().points() / 2.0).max(0.0);

        xml::push_fmt(
            &mut self.generated,
            format_args!(
                "<style:style style:name=\"{name}\" style:family=\"table\"><style:table-properties style:width=\"100%\" table:align=\"margins\" table:border-model=\"collapsing\" fo:border=\"{border} solid {color}\"/></style:style>\n\
                 <style:style style:name=\"{name}.Column\" style:family=\"table-column\"><style:table-column-properties style:column-width=\"{width}\"/></style:style>\n\
                 <style:style style:name=\"{name}.Cell\" style:family=\"table-cell\"><style:table-cell-properties fo:padding-left=\"{left}\" fo:padding-right=\"{left}\" fo:padding-top=\"{top}\" fo:padding-bottom=\"{top}\" fo:border=\"{border} solid {color}\"/></style:style>\n",
                width = xml::centimetres_points(column_width),
                left = xml::points_value(horizontal),
                top = xml::points_value(vertical),
            ),
        );
        if let Some(fill) = style.header_fill() {
            xml::push_fmt(
                &mut self.generated,
                format_args!(
                    "<style:style style:name=\"{name}.Header\" style:family=\"table-cell\"><style:table-cell-properties fo:padding-left=\"{left}\" fo:padding-right=\"{left}\" fo:padding-top=\"{top}\" fo:padding-bottom=\"{top}\" fo:border=\"{border} solid {color}\" fo:background-color=\"{fill}\"/></style:style>\n",
                    left = xml::points_value(horizontal),
                    top = xml::points_value(vertical),
                    fill = fill.hex(),
                ),
            );
        }

        xml::push_fmt(
            &mut self.body,
            format_args!(
                "<table:table table:name=\"{name}\" table:style-name=\"{name}\">\n\
                 <table:table-column table:style-name=\"{name}.Column\" table:number-columns-repeated=\"{columns}\"/>\n\
                 <table:table-header-rows><table:table-row>\n"
            ),
        );
        for (cell, alignment) in table.header().iter().zip(table.alignments()) {
            self.cell(cell, *alignment, true, &name);
        }
        xml::push(
            &mut self.body,
            "</table:table-row></table:table-header-rows>\n",
        );
        for row in table.rows() {
            xml::push(&mut self.body, "<table:table-row>\n");
            for (cell, alignment) in row.iter().zip(table.alignments()) {
                self.cell(cell, *alignment, false, &name);
            }
            xml::push(&mut self.body, "</table:table-row>\n");
        }
        xml::push(&mut self.body, "</table:table>\n");
    }

    fn cell(&mut self, cell: &TableCell, alignment: ColumnAlignment, header: bool, table: &str) {
        let style = if header && self.template.table().header_fill().is_some() {
            format!("{table}.Header")
        } else {
            format!("{table}.Cell")
        };
        let paragraph = match alignment {
            ColumnAlignment::Default => "CellDefault",
            ColumnAlignment::Left => "CellLeft",
            ColumnAlignment::Center => "CellCenter",
            ColumnAlignment::Right => "CellRight",
        };
        xml::push_fmt(
            &mut self.body,
            format_args!(
                "<table:table-cell table:style-name=\"{style}\" office:value-type=\"string\">"
            ),
        );
        self.paragraph_with(paragraph, cell.content());
        xml::push(&mut self.body, "</table:table-cell>\n");
    }

    fn quote(&mut self, quote: &Quote) {
        self.flush_break_as_paragraph();
        if let QuoteKind::Alert(alert) = quote.kind() {
            self.paragraph_with(
                "QuoteParagraph",
                &[Inline::Text(format!("[{}]", alert_label(alert)))],
            );
        }
        for block in quote.blocks() {
            match block {
                Block::Paragraph(inlines) => self.paragraph_with("QuoteParagraph", inlines),
                Block::Heading(heading) => {
                    self.paragraph_with("QuoteParagraph", heading.content());
                }
                _ => self.block(block),
            }
        }
    }

    fn code(&mut self, code: &CodeBlock) {
        let style = self.style_with_break("Code");
        self.open_paragraph(&style);
        let text = code.text().trim_end_matches('\n');
        for (index, line) in text.lines().enumerate() {
            if index > 0 {
                xml::push(&mut self.body, "<text:line-break/>");
            }
            self.code_line(line);
        }
        self.close_paragraph();
    }

    fn code_line(&mut self, line: &str) {
        let mut spaces = 0_usize;
        for character in line.chars() {
            match character {
                ' ' => spaces += 1,
                '\t' => {
                    self.flush_spaces(&mut spaces);
                    xml::push(&mut self.body, "<text:tab/>");
                }
                other => {
                    self.flush_spaces(&mut spaces);
                    let mut buffer = [0_u8; 4];
                    self.text(other.encode_utf8(&mut buffer));
                }
            }
        }
        self.flush_spaces(&mut spaces);
    }

    fn flush_spaces(&mut self, spaces: &mut usize) {
        let count = std::mem::take(spaces);
        if count > 0 {
            xml::push_fmt(&mut self.body, format_args!("<text:s text:c=\"{count}\"/>"));
        }
    }

    fn inlines(&mut self, inlines: &[Inline]) {
        for inline in inlines {
            self.inline(inline);
        }
    }

    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Text(text) => self.text(text),
            Inline::Code(code) => self.span("CodeInline", code),
            Inline::Emphasis(content) => self.span_with("Emphasis", content),
            Inline::Strong(content) => self.span_with("Strong", content),
            Inline::Strikethrough(content) => self.span_with("Strikethrough", content),
            Inline::Link {
                destination,
                content,
                ..
            } => {
                xml::push_fmt(
                    &mut self.body,
                    format_args!(
                        "<text:a xlink:type=\"simple\" xlink:href=\"{}\">",
                        xml::escape_attribute(destination.as_str())
                    ),
                );
                if content.is_empty() {
                    self.text(destination.as_str());
                } else {
                    self.inlines(content);
                }
                xml::push(&mut self.body, "</text:a>");
            }
            Inline::Image {
                destination,
                alternative,
                ..
            } => self.image(destination, alternative),
            Inline::SoftBreak => self.text(" "),
            Inline::HardBreak => xml::push(&mut self.body, "<text:line-break/>"),
            Inline::FootnoteReference(id) => self.note(id),
        }
    }

    fn text(&mut self, text: &str) {
        xml::push(&mut self.body, &xml::escape_text(text));
    }

    fn span(&mut self, style: &str, text: &str) {
        xml::push_fmt(
            &mut self.body,
            format_args!(
                "<text:span text:style-name=\"{style}\">{}</text:span>",
                xml::escape_text(text)
            ),
        );
    }

    fn span_with(&mut self, style: &str, content: &[Inline]) {
        xml::push_fmt(
            &mut self.body,
            format_args!("<text:span text:style-name=\"{style}\">"),
        );
        self.inlines(content);
        xml::push(&mut self.body, "</text:span>");
    }

    fn image(&mut self, destination: &Destination, alternative: &[Inline]) {
        let label = plain_text(alternative);
        let Some(virtual_name) = self.assets.map().image(destination.as_str()) else {
            self.text(&format!("[{label}]"));
            return;
        };
        let file = picture_name(virtual_name);
        let (width, height) = match self.assets.files().get(virtual_name) {
            Some(bytes) => crate::image::dimensions(bytes, self.text_width),
            None => (self.text_width * 0.5, self.text_width * 0.25),
        };
        self.image_count += 1;
        let name = format!("Image{}", self.image_count);
        xml::push_fmt(
            &mut self.body,
            format_args!(
                "<draw:frame draw:name=\"{name}\" text:anchor-type=\"as-char\" svg:width=\"{width}\" svg:height=\"{height}\">\
                 <draw:image xlink:href=\"{href}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/>\
                 {description}</draw:frame>",
                width = xml::centimetres_points(width),
                height = xml::centimetres_points(height),
                href = xml::escape_attribute(&format!("Pictures/{file}")),
                description = if label.is_empty() {
                    String::new()
                } else {
                    format!("<svg:desc>{}</svg:desc>", xml::escape_text(&label))
                },
            ),
        );
    }

    fn note(&mut self, id: &FootnoteId) {
        let key = id.as_str();
        if self.active.iter().any(|active| active == key) {
            self.text(&format!("[^{key}]"));
            return;
        }
        let Some(definition) = self.definitions.get(key).cloned() else {
            self.text(&format!("[^{key}]"));
            return;
        };
        self.note_count += 1;
        let number = self.note_count;
        xml::push_fmt(
            &mut self.body,
            format_args!(
                "<text:note text:note-class=\"footnote\" text:id=\"ftn{number}\"><text:note-citation>{number}</text:note-citation><text:note-body>"
            ),
        );
        self.active.push(key.to_owned());
        self.blocks(definition.blocks());
        self.active.pop();
        xml::push(&mut self.body, "</text:note-body></text:note>");
    }
}

fn cell_paragraph_styles(output: &mut String, template: &Template) {
    let paragraph = template.formatting().paragraph();
    let line = crate::metrics::line_advance(paragraph.face().size(), paragraph.leading());
    for (name, alignment) in [
        ("CellDefault", "start"),
        ("CellLeft", "left"),
        ("CellCenter", "center"),
        ("CellRight", "right"),
    ] {
        xml::push_fmt(
            output,
            format_args!(
                "<style:style style:name=\"{name}\" style:family=\"paragraph\"><style:paragraph-properties fo:line-height=\"{line}\" fo:margin-bottom=\"0cm\" fo:hyphenate=\"true\" fo:text-align=\"{alignment}\"/></style:style>\n",
                line = xml::points_value(line),
            ),
        );
    }
}

/// The code and quote paragraph styles, patched by the template where it declares one.
fn block_styles(output: &mut String, template: &Template) {
    let formatting = template.formatting();
    let code_style = formatting.code_block();
    let code = partial_properties(
        code_style,
        Some("fo:font-family=\"'Liberation Mono', monospace\""),
    );
    let code_background = background_attribute(code_style.and_then(PartialTextStyle::background));
    xml::push_fmt(
        output,
        format_args!(
            "<style:style style:name=\"Code\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties{code_background}/><style:text-properties {code}/></style:style>\n<style:style style:name=\"CodeBreak\" style:family=\"paragraph\" style:parent-style-name=\"Code\"><style:paragraph-properties fo:break-before=\"page\"/></style:style>\n"
        ),
    );
    if formatting.paragraph().background().is_some() {
        xml::push(
            output,
            "<style:style style:name=\"BodyParagraphBreak\" style:family=\"paragraph\" style:parent-style-name=\"BodyParagraph\"><style:paragraph-properties fo:break-before=\"page\"/></style:style>\n",
        );
    }
    let quote_style = formatting.quote();
    let quote = partial_properties(quote_style, None);
    let quote_background = background_attribute(quote_style.and_then(PartialTextStyle::background));
    let text = if quote.is_empty() {
        String::new()
    } else {
        format!("<style:text-properties {quote}/>")
    };
    xml::push_fmt(
        output,
        format_args!(
            "<style:style style:name=\"QuoteParagraph\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties fo:margin-left=\"1cm\" fo:margin-right=\"1cm\"{quote_background}/>{text}</style:style>\n"
        ),
    );
}

fn horizontal_line_style(output: &mut String, template: &Template) {
    let rule = template.formatting().rule();
    let before = rule.spacing_before().map_or_else(String::new, |length| {
        format!("fo:margin-top=\"{}\" ", xml::points(length))
    });
    let after = rule.spacing_after().map_or_else(String::new, |length| {
        format!("fo:margin-bottom=\"{}\" ", xml::points(length))
    });
    xml::push_fmt(
        output,
        format_args!(
            "<style:style style:name=\"HorizontalLine\" style:family=\"paragraph\" style:parent-style-name=\"Standard\"><style:paragraph-properties {before}{after}fo:border-bottom=\"{thickness} solid {color}\" fo:padding-bottom=\"0.1cm\"/></style:style>\n",
            before = before,
            after = after,
            thickness = xml::points(rule.thickness()),
            color = rule.color().hex(),
        ),
    );
}

fn alert_label(alert: Alert) -> &'static str {
    match alert {
        Alert::Note => "!NOTE",
        Alert::Tip => "!TIP",
        Alert::Important => "!IMPORTANT",
        Alert::Warning => "!WARNING",
        Alert::Caution => "!CAUTION",
    }
}

fn plain_text(inlines: &[Inline]) -> String {
    let mut output = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) => output.push_str(text),
            Inline::Emphasis(content)
            | Inline::Strong(content)
            | Inline::Strikethrough(content) => output.push_str(&plain_text(content)),
            Inline::Link { content, .. } => output.push_str(&plain_text(content)),
            Inline::Image { alternative, .. } => output.push_str(&plain_text(alternative)),
            Inline::SoftBreak | Inline::HardBreak => output.push(' '),
            Inline::FootnoteReference(id) => output.push_str(&format!("[^{}]", id.as_str())),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Metadata;

    #[test]
    fn alert_labels_are_uppercase() {
        assert_eq!(alert_label(Alert::Warning), "!WARNING");
    }

    #[test]
    fn an_empty_document_still_has_a_body() {
        let document = Document::new(Metadata::default(), Vec::new());
        let template = Template::embedded().expect("the embedded template must be valid");

        let content = content_xml(&document, &template, &Assets::default());

        assert!(content.contains("<office:text>"));
        assert!(!content.contains("<text:p"));
    }
}
