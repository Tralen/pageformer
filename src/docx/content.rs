//! Generates `word/document.xml`, the header and footer and the footnotes.

use std::collections::BTreeMap;

use crate::assets::Assets;
use crate::document::{
    Block, CodeBlock, ColumnAlignment, Destination, Document, FootnoteDefinition, FootnoteId,
    Inline, List, ListItem, ListStart, Quote, QuoteKind, Table, TableCell, TaskState,
};
use crate::image;
use crate::metrics;
use crate::template::Template;
use crate::xml;

use super::numbering::{Numbering, NumberingKind};
use super::styles::partial_properties;

/// A packaged image and the relationship that points to it.
pub(crate) struct Media {
    pub virtual_name: String,
    pub package_path: String,
    pub relationship: String,
}

/// Plans the media entries in a stable order.
pub(crate) fn media_plan(assets: &Assets) -> Vec<Media> {
    assets
        .files()
        .keys()
        .enumerate()
        .map(|(index, virtual_name)| Media {
            virtual_name: virtual_name.clone(),
            package_path: format!("word/media/{}", file_name(virtual_name)),
            relationship: format!("rId{}", 7_usize.saturating_add(index)),
        })
        .collect()
}

/// The inline spans whose face can carry a color.
enum Span {
    Emphasis,
    Strong,
}

fn foreground_run(template: &Template, span: Span) -> String {
    let paragraph = template.formatting().paragraph();
    let inline = match span {
        Span::Emphasis => paragraph.emphasis(),
        Span::Strong => paragraph.strong(),
    };
    let mut output = String::new();
    if let Some(color) = inline.face().foreground() {
        output.push_str(&format!(
            "<w:color w:val=\"{}\"/>",
            color.hex().trim_start_matches('#')
        ));
    }
    if let Some(color) = inline.background() {
        output.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            color.hex().trim_start_matches('#')
        ));
    }
    output
}

fn file_name(virtual_name: &str) -> &str {
    match virtual_name.rsplit_once('/') {
        Some((_, name)) => name,
        None => virtual_name,
    }
}

/// The parts the content builder produced.
pub(crate) struct Content {
    pub document: String,
    pub header: Option<String>,
    pub footer: Option<String>,
    pub footnotes: String,
    pub hyperlinks: Vec<String>,
    pub numbering: Numbering,
}

/// Builds the content parts.
pub(crate) fn build(
    document: &Document,
    template: &Template,
    assets: &Assets,
    media: &[Media],
) -> Content {
    let mut builder = Builder::new(template, assets, media);
    builder.collect_definitions(document.blocks());
    builder.blocks(document.blocks(), 0);
    let sect = builder.section_properties();
    let mut body = String::new();
    xml::push(
        &mut body,
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\n\
         <w:body>\n",
    );
    xml::push(&mut body, &builder.document);
    xml::push(&mut body, &sect);
    xml::push(&mut body, "</w:body>\n</w:document>\n");

    let footnotes = builder.footnotes_xml();
    let header = header_xml(document, template, assets, media);
    let footer = footer_xml(document, template);
    Content {
        document: body,
        header,
        footer,
        footnotes,
        hyperlinks: builder.hyperlinks,
        numbering: builder.numbering,
    }
}

struct Builder<'a> {
    document: String,
    footnotes: Vec<String>,
    numbering: Numbering,
    hyperlinks: Vec<String>,
    hyperlink_ids: BTreeMap<String, usize>,
    definitions: BTreeMap<String, FootnoteDefinition>,
    active: Vec<String>,
    note_count: usize,
    image_count: usize,
    text_width: f64,
    template: &'a Template,
    assets: &'a Assets,
    media: &'a [Media],
}

impl<'a> Builder<'a> {
    fn new(template: &'a Template, assets: &'a Assets, media: &'a [Media]) -> Self {
        let page = template.page();
        let dimensions = page.dimensions();
        let margins = page.margins();
        let text_width =
            (dimensions.width().points() - margins.left().points() - margins.right().points())
                .max(1.0);
        Self {
            document: String::new(),
            footnotes: Vec::new(),
            numbering: Numbering::new(),
            hyperlinks: Vec::new(),
            hyperlink_ids: BTreeMap::new(),
            definitions: BTreeMap::new(),
            active: Vec::new(),
            note_count: 0,
            image_count: 0,
            text_width,
            template,
            assets,
            media,
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

    fn blocks(&mut self, blocks: &[Block], depth: usize) {
        for block in blocks {
            self.block(block, depth);
        }
    }

    fn block(&mut self, block: &Block, depth: usize) {
        match block {
            Block::Heading(heading) => {
                let level = heading.level().get();
                let runs = self.runs(heading.content(), "");
                let paragraph = self.paragraph(Some(&format!("Heading{level}")), "", &runs);
                self.document.push_str(&paragraph);
            }
            Block::Paragraph(inlines) => {
                let runs = self.runs(inlines, "");
                let paragraph = self.paragraph(self.body_style(), "", &runs);
                self.document.push_str(&paragraph);
            }
            Block::NumberedParagraph(paragraph) => {
                let mut runs = self.run(&format!("{}. ", paragraph.number()), "");
                runs.push_str(&self.runs(paragraph.content(), ""));
                let output = self.paragraph(self.body_style(), "", &runs);
                self.document.push_str(&output);
            }
            Block::List(list) => self.list(list, depth),
            Block::Table(table) => self.table(table),
            Block::Quote(quote) => self.quote(quote),
            Block::CodeBlock(code) => self.code(code),
            Block::ThematicBreak => {
                let line = self.paragraph(None, &self.thematic_break_properties(), "");
                self.document.push_str(&line);
            }
            Block::PageBreak => self
                .document
                .push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>\n"),
            Block::FootnoteDefinition(_) => {}
        }
    }

    /// The bottom border that draws the rule, plus the spacing the template declares.
    fn thematic_break_properties(&self) -> String {
        let rule = self.template.formatting().rule();
        let mut output = format!(
            "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"{}\" w:space=\"1\" w:color=\"{}\"/></w:pBdr>",
            metrics::eighths(rule.thickness().points()),
            rule.color().hex().trim_start_matches('#')
        );
        let before = rule.spacing_before().map_or_else(String::new, |length| {
            format!(" w:before=\"{}\"", metrics::twips(length.points()))
        });
        let after = rule.spacing_after().map_or_else(String::new, |length| {
            format!(" w:after=\"{}\"", metrics::twips(length.points()))
        });
        if !before.is_empty() || !after.is_empty() {
            output.push_str(&format!("<w:spacing{before}{after}/>"));
        }
        output
    }

    /// Body paragraphs take the fill through their own style, so quotes and lists
    /// keep the background their own style declares.
    fn body_style(&self) -> Option<&'static str> {
        if self
            .template
            .formatting()
            .paragraph()
            .background()
            .is_some()
        {
            Some("BodyParagraph")
        } else {
            None
        }
    }

    fn paragraph(&self, style: Option<&str>, extra: &str, runs: &str) -> String {
        let mut output = String::new();
        xml::push(&mut output, "<w:p><w:pPr>");
        if let Some(style) = style {
            xml::push_fmt(
                &mut output,
                format_args!("<w:pStyle w:val=\"{}\"/>", xml::escape_attribute(style)),
            );
        }
        xml::push(&mut output, extra);
        xml::push(&mut output, "</w:pPr>");
        xml::push(&mut output, runs);
        xml::push(&mut output, "</w:p>\n");
        output
    }

    fn run(&self, text: &str, properties: &str) -> String {
        if properties.is_empty() {
            format!(
                "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml::escape_text(text)
            )
        } else {
            format!(
                "<w:r><w:rPr>{properties}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml::escape_text(text)
            )
        }
    }

    fn runs(&mut self, inlines: &[Inline], properties: &str) -> String {
        let mut output = String::new();
        for inline in inlines {
            output.push_str(&self.inline(inline, properties));
        }
        output
    }

    fn inline(&mut self, inline: &Inline, properties: &str) -> String {
        match inline {
            Inline::Text(text) => self.run(text, properties),
            Inline::Code(code) => {
                let mut merged = String::from("<w:rStyle w:val=\"CodeChar\"/>");
                merged.push_str(properties);
                self.run(code, &merged)
            }
            Inline::Emphasis(content) => {
                let mut merged = String::from("<w:i/>");
                merged.push_str(&foreground_run(self.template, Span::Emphasis));
                merged.push_str(properties);
                self.runs(content, &merged)
            }
            Inline::Strong(content) => {
                let mut merged = String::from("<w:b/>");
                merged.push_str(&foreground_run(self.template, Span::Strong));
                merged.push_str(properties);
                self.runs(content, &merged)
            }
            Inline::Strikethrough(content) => {
                let mut merged = String::from("<w:strike/>");
                merged.push_str(&partial_properties(
                    Some(self.template.formatting().paragraph().strikethrough()),
                    None,
                    true,
                ));
                merged.push_str(properties);
                self.runs(content, &merged)
            }
            Inline::Link {
                destination,
                content,
                ..
            } => {
                let id = self.hyperlink(destination.as_str());
                let mut merged = String::from("<w:rStyle w:val=\"Hyperlink\"/>");
                merged.push_str(properties);
                let label = if content.is_empty() {
                    self.run(destination.as_str(), &merged)
                } else {
                    self.runs(content, &merged)
                };
                format!("<w:hyperlink r:id=\"{id}\">{label}</w:hyperlink>")
            }
            Inline::Image {
                destination,
                alternative,
                ..
            } => self.image(destination, alternative),
            Inline::SoftBreak => self.run(" ", properties),
            Inline::HardBreak => "<w:r><w:br/></w:r>".to_owned(),
            Inline::FootnoteReference(id) => self.note(id),
        }
    }

    fn hyperlink(&mut self, destination: &str) -> String {
        if let Some(index) = self.hyperlink_ids.get(destination) {
            return hyperlink_relationship(self.media.len(), *index);
        }
        self.hyperlinks.push(destination.to_owned());
        let index = self.hyperlinks.len();
        self.hyperlink_ids.insert(destination.to_owned(), index);
        hyperlink_relationship(self.media.len(), index)
    }

    fn image(&mut self, destination: &Destination, alternative: &[Inline]) -> String {
        let label = plain_text(alternative);
        let Some(virtual_name) = self.assets.map().image(destination.as_str()) else {
            return self.run(&format!("[{label}]"), "");
        };
        let Some(media) = self
            .media
            .iter()
            .find(|media| media.virtual_name == virtual_name)
        else {
            return self.run(&format!("[{label}]"), "");
        };
        let (width, height) = match self.assets.files().get(virtual_name) {
            Some(bytes) => image::dimensions(bytes, self.text_width),
            None => (self.text_width * 0.5, self.text_width * 0.25),
        };
        self.image_count += 1;
        let id = self.image_count;
        drawing_inline(
            &media.relationship,
            id,
            width,
            height,
            if label.is_empty() { None } else { Some(&label) },
        )
    }

    fn note(&mut self, id: &FootnoteId) -> String {
        let key = id.as_str();
        if self.active.iter().any(|active| active == key) {
            return self.run(&format!("[^{key}]"), "");
        }
        let Some(definition) = self.definitions.get(key).cloned() else {
            return self.run(&format!("[^{key}]"), "");
        };
        self.note_count += 1;
        let number = self.note_count;
        self.active.push(key.to_owned());
        let mut body = String::new();
        for (index, block) in definition.blocks().iter().enumerate() {
            let runs = match block {
                Block::Paragraph(inlines) => self.runs(inlines, ""),
                Block::NumberedParagraph(paragraph) => {
                    let mut runs = self.run(&format!("{}. ", paragraph.number()), "");
                    runs.push_str(&self.runs(paragraph.content(), ""));
                    runs
                }
                Block::CodeBlock(code) => self.run(code.text().trim_end_matches('\n'), ""),
                _ => String::new(),
            };
            let mark = if index == 0 {
                "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteRef/></w:r>"
            } else {
                ""
            };
            xml::push_fmt(
                &mut body,
                format_args!(
                    "<w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr>{mark}{runs}</w:p>"
                ),
            );
        }
        self.active.pop();
        self.footnotes
            .push(format!("<w:footnote w:id=\"{number}\">{body}</w:footnote>"));
        format!(
            "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteReference w:id=\"{number}\"/></w:r>"
        )
    }

    fn list(&mut self, list: &List, depth: usize) {
        let (kind, start) = match list.start() {
            ListStart::Unordered => (NumberingKind::Bullet, 1),
            ListStart::Ordered(start) => (NumberingKind::Decimal, start),
        };
        let number = self.numbering.add(kind, start, depth);
        let marker =
            format!("<w:numPr><w:ilvl w:val=\"{depth}\"/><w:numId w:val=\"{number}\"/></w:numPr>");
        for (index, item) in list.items().iter().enumerate() {
            let last = index.saturating_add(1) == list.items().len();
            self.item(item, &marker, depth, list.is_tight() && !last);
        }
    }

    fn item(&mut self, item: &ListItem, marker: &str, depth: usize, tight: bool) {
        let extra = if tight {
            format!("{marker}<w:spacing w:after=\"0\"/>")
        } else {
            marker.to_owned()
        };
        let task = match item.task() {
            Some(TaskState::Done) => "[x] ",
            Some(TaskState::Todo) => "[ ] ",
            None => "",
        };
        match item.blocks().first() {
            Some(Block::Paragraph(inlines)) => {
                let mut runs = self.run(task, "");
                runs.push_str(&self.runs(inlines, ""));
                let paragraph = self.paragraph(None, &extra, &runs);
                self.document.push_str(&paragraph);
                if let Some(rest) = item.blocks().get(1..) {
                    self.blocks(rest, depth.saturating_add(1));
                }
            }
            _ => {
                if !task.is_empty() {
                    let runs = self.run(task.trim_end(), "");
                    let paragraph = self.paragraph(None, &extra, &runs);
                    self.document.push_str(&paragraph);
                }
                self.blocks(item.blocks(), depth.saturating_add(1));
            }
        }
    }

    fn table(&mut self, table: &Table) {
        let style = self.template.table();
        let columns = table.alignments().len().max(1);
        let column_width = self.text_width / columns as f64;
        let paragraph = self.template.formatting().paragraph();
        let horizontal = style.cell_padding().points();
        let vertical = (horizontal - paragraph.leading().points() / 2.0).max(0.0);
        let border = if style.border().points() > 0.0 {
            format!(
                "w:val=\"single\" w:sz=\"{}\" w:space=\"0\" w:color=\"{}\"",
                metrics::eighths(style.border().points()),
                style.border_color().hex().trim_start_matches('#')
            )
        } else {
            "w:val=\"nil\"".to_owned()
        };
        let mut output = String::new();
        xml::push(
            &mut output,
            "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblLayout w:type=\"fixed\"/>",
        );
        xml::push_fmt(
            &mut output,
            format_args!(
                "<w:tblBorders><w:top {border}/><w:left {border}/><w:bottom {border}/><w:right {border}/><w:insideH {border}/><w:insideV {border}/></w:tblBorders>"
            ),
        );
        xml::push_fmt(
            &mut output,
            format_args!(
                "<w:tblCellMar><w:top w:w=\"{top}\" w:type=\"dxa\"/><w:left w:w=\"{left}\" w:type=\"dxa\"/><w:bottom w:w=\"{top}\" w:type=\"dxa\"/><w:right w:w=\"{left}\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr>",
                top = metrics::twips(vertical),
                left = metrics::twips(horizontal),
            ),
        );
        xml::push(&mut output, "<w:tblGrid>");
        for _ in 0..columns {
            xml::push_fmt(
                &mut output,
                format_args!("<w:gridCol w:w=\"{}\"/>", metrics::twips(column_width)),
            );
        }
        xml::push(&mut output, "</w:tblGrid>");

        xml::push(&mut output, "<w:tr><w:trPr><w:tblHeader/></w:trPr>");
        for (cell, alignment) in table.header().iter().zip(table.alignments()) {
            self.cell(
                &mut output,
                cell,
                *alignment,
                style.header_fill(),
                column_width,
            );
        }
        xml::push(&mut output, "</w:tr>");
        for row in table.rows() {
            xml::push(&mut output, "<w:tr>");
            for (cell, alignment) in row.iter().zip(table.alignments()) {
                self.cell(&mut output, cell, *alignment, None, column_width);
            }
            xml::push(&mut output, "</w:tr>");
        }
        xml::push(&mut output, "</w:tbl>\n");
        self.document.push_str(&output);
    }

    fn cell(
        &mut self,
        output: &mut String,
        cell: &TableCell,
        alignment: ColumnAlignment,
        fill: Option<crate::template::Color>,
        width: f64,
    ) {
        xml::push_fmt(
            output,
            format_args!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/>",
                metrics::twips(width)
            ),
        );
        if let Some(fill) = fill {
            xml::push_fmt(
                output,
                format_args!(
                    "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                    fill.hex().trim_start_matches('#')
                ),
            );
        }
        xml::push(output, "<w:vAlign w:val=\"center\"/></w:tcPr>");
        let runs = self.runs(cell.content(), "");
        let justify = match alignment {
            ColumnAlignment::Default | ColumnAlignment::Left => "left",
            ColumnAlignment::Center => "center",
            ColumnAlignment::Right => "right",
        };
        let paragraph = self.template.formatting().paragraph();
        let line = metrics::line_advance(paragraph.face().size(), paragraph.leading());
        xml::push_fmt(
            output,
            format_args!(
                "<w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"{}\" w:lineRule=\"atLeast\"/><w:jc w:val=\"{justify}\"/></w:pPr>{runs}</w:p>",
                metrics::twips(line)
            ),
        );
        xml::push(output, "</w:tc>");
    }

    fn quote(&mut self, quote: &Quote) {
        if let QuoteKind::Alert(alert) = quote.kind() {
            let runs = self.run(&format!("[{}]", alert.label()), "");
            let paragraph = self.paragraph(Some("Quote"), "", &runs);
            self.document.push_str(&paragraph);
        }
        for block in quote.blocks() {
            match block {
                Block::Paragraph(inlines) => {
                    let runs = self.runs(inlines, "");
                    let paragraph = self.paragraph(Some("Quote"), "", &runs);
                    self.document.push_str(&paragraph);
                }
                Block::Heading(heading) => {
                    let runs = self.runs(heading.content(), "");
                    let paragraph = self.paragraph(Some("Quote"), "", &runs);
                    self.document.push_str(&paragraph);
                }
                _ => self.block(block, 0),
            }
        }
    }

    fn code(&mut self, code: &CodeBlock) {
        let text = code.text().trim_end_matches('\n');
        let mut runs = String::new();
        for (index, line) in text.lines().enumerate() {
            if index > 0 {
                runs.push_str("<w:r><w:br/></w:r>");
            }
            runs.push_str(&self.run(line, ""));
        }
        let paragraph = self.paragraph(Some("Code"), "", &runs);
        self.document.push_str(&paragraph);
    }

    fn footnotes_xml(&self) -> String {
        let mut output = String::new();
        xml::push(
            &mut output,
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:footnotes xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n\
             <w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>\n\
             <w:footnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>\n",
        );
        for note in &self.footnotes {
            xml::push(&mut output, note);
            xml::push(&mut output, "\n");
        }
        xml::push(&mut output, "</w:footnotes>\n");
        output
    }

    fn section_properties(&self) -> String {
        let page = self.template.page();
        let dimensions = page.dimensions();
        let margins = page.margins();
        let header = if page.header().is_some() || self.assets.map().letterhead().is_some() {
            "<w:headerReference w:type=\"default\" r:id=\"rId5\"/>"
        } else {
            ""
        };
        let footer = if page.footer().is_some() {
            "<w:footerReference w:type=\"default\" r:id=\"rId6\"/>"
        } else {
            ""
        };
        format!(
            "<w:sectPr>{header}{footer}<w:pgSz w:w=\"{width}\" w:h=\"{height}\"/><w:pgMar w:top=\"{top}\" w:right=\"{right}\" w:bottom=\"{bottom}\" w:left=\"{left}\" w:header=\"530\" w:footer=\"567\" w:gutter=\"0\"/></w:sectPr>\n",
            width = metrics::twips(dimensions.width().points()),
            height = metrics::twips(dimensions.height().points()),
            top = metrics::twips(margins.top().points()),
            right = metrics::twips(margins.right().points()),
            bottom = metrics::twips(margins.bottom().points()),
            left = metrics::twips(margins.left().points()),
        )
    }
}

fn header_xml(
    document: &Document,
    template: &Template,
    assets: &Assets,
    media: &[Media],
) -> Option<String> {
    let page = template.page();
    if page.header().is_none() && assets.map().letterhead().is_none() {
        return None;
    }
    let mut output = header_prefix();
    if let Some(letterhead) = assets.map().letterhead()
        && let Some(media) = media.iter().find(|media| media.virtual_name == letterhead)
        && let Some(bytes) = assets.files().get(letterhead)
    {
        let dimensions = page.dimensions();
        let mut width = dimensions.width().points();
        let mut height = image::dimensions(bytes, width).1;
        if height > dimensions.height().points() {
            let scale = dimensions.height().points() / height;
            height *= scale;
            width *= scale;
        }
        xml::push(
            &mut output,
            "<w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr>",
        );
        xml::push(
            &mut output,
            &drawing_anchor(&media.relationship, width, height),
        );
        xml::push(&mut output, "</w:p>\n");
    }
    if let Some(header) = page.header() {
        xml::push(&mut output, &running_paragraph(header, template, document));
    }
    xml::push(&mut output, "</w:hdr>\n");
    Some(output)
}

fn footer_xml(document: &Document, template: &Template) -> Option<String> {
    let page = template.page();
    let footer = page.footer()?;
    let mut output = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:ftr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n",
    );
    xml::push(&mut output, &running_paragraph(footer, template, document));
    xml::push(&mut output, "</w:ftr>\n");
    Some(output)
}

fn header_prefix() -> String {
    String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\n",
    )
}

fn running_paragraph(
    running: &crate::template::RunningText,
    template: &Template,
    document: &Document,
) -> String {
    let page = template.page();
    let dimensions = page.dimensions();
    let margins = page.margins();
    let width =
        (dimensions.width().points() - margins.left().points() - margins.right().points()).max(1.0);
    let slots = running.slots();
    let mut output = String::new();
    xml::push_fmt(
        &mut output,
        format_args!(
            "<w:p><w:pPr><w:pStyle w:val=\"Normal\"/><w:tabs><w:tab w:val=\"center\" w:pos=\"{}\"/><w:tab w:val=\"right\" w:pos=\"{}\"/></w:tabs></w:pPr>",
            metrics::twips(width / 2.0),
            metrics::twips(width)
        ),
    );
    slot_runs(&mut output, slots.left(), document);
    xml::push(&mut output, "<w:r><w:tab/></w:r>");
    slot_runs(&mut output, slots.center(), document);
    xml::push(&mut output, "<w:r><w:tab/></w:r>");
    slot_runs(&mut output, slots.right(), document);
    xml::push(&mut output, "</w:p>\n");
    output
}

fn slot_runs(output: &mut String, text: Option<&str>, document: &Document) {
    let Some(text) = text else {
        return;
    };
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        xml::push_fmt(
            output,
            format_args!(
                "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml::escape_text(rest.get(..start).unwrap_or_default())
            ),
        );
        let after = rest.get(start.saturating_add(1)..).unwrap_or_default();
        let Some(end) = after.find('}') else {
            xml::push_fmt(
                output,
                format_args!(
                    "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml::escape_text(rest.get(start..).unwrap_or_default())
                ),
            );
            return;
        };
        let token = after.get(..end).unwrap_or_default();
        match token {
            "page" => xml::push(
                output,
                "<w:fldSimple w:instr=\" PAGE \"><w:r><w:t>1</w:t></w:r></w:fldSimple>",
            ),
            "pages" => xml::push(
                output,
                "<w:fldSimple w:instr=\" NUMPAGES \"><w:r><w:t>1</w:t></w:r></w:fldSimple>",
            ),
            "title" => xml::push_fmt(
                output,
                format_args!(
                    "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml::escape_text(
                        document
                            .metadata()
                            .title()
                            .map(|value| value.as_str())
                            .unwrap_or_default()
                    )
                ),
            ),
            "author" => xml::push_fmt(
                output,
                format_args!(
                    "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml::escape_text(
                        document
                            .metadata()
                            .author()
                            .map(|value| value.as_str())
                            .unwrap_or_default()
                    )
                ),
            ),
            other => xml::push_fmt(
                output,
                format_args!(
                    "<w:r><w:t xml:space=\"preserve\">{{{}}}</w:t></w:r>",
                    xml::escape_text(other)
                ),
            ),
        }
        rest = after.get(end.saturating_add(1)..).unwrap_or_default();
    }
    xml::push_fmt(
        output,
        format_args!(
            "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
            xml::escape_text(rest)
        ),
    );
}

fn drawing_inline(
    relationship: &str,
    id: usize,
    width: f64,
    height: f64,
    description: Option<&str>,
) -> String {
    let cx = metrics::emu(width);
    let cy = metrics::emu(height);
    let description = match description {
        Some(text) => format!(" descr=\"{}\"", xml::escape_attribute(text)),
        None => String::new(),
    };
    format!(
        "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{id}\" name=\"Image{id}\"{description}/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Image{id}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{relationship}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"
    )
}

fn drawing_anchor(relationship: &str, width: f64, height: f64) -> String {
    let cx = metrics::emu(width);
    let cy = metrics::emu(height);
    format!(
        "<w:r><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"0\" behindDoc=\"1\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"page\"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"page\"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:wrapNone/><wp:docPr id=\"1\" name=\"Letterhead\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"1\" name=\"Letterhead\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{relationship}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"
    )
}

pub(crate) fn hyperlink_relationship(media: usize, index: usize) -> String {
    format!("rId{}", 8_usize.saturating_add(media).saturating_add(index))
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
