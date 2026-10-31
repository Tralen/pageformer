//! Converts the document model into the HTML body.

use std::collections::BTreeMap;

use crate::assets::{Assets, media_type};
use crate::document::{
    Alert, Block, CodeBlock, ColumnAlignment, Destination, Document, FootnoteDefinition,
    FootnoteId, ImageBlock, Inline, List, ListItem, ListStart, Quote, QuoteKind, Table, TableCell,
    TaskState,
};
use crate::html::data_uri::data_uri;
use crate::html::escape;
use crate::image::{self, ImageAlignment, ImageDimension, ImageSize};
use crate::template::Template;

/// The rendered body, its endnotes and the warnings it produced.
pub(crate) struct Content {
    pub body: String,
    pub footnotes: String,
    pub warnings: Vec<String>,
}

impl Content {
    /// The body and the endnotes as one fragment, matching the HTML file.
    pub(crate) fn full(&self) -> String {
        if self.footnotes.is_empty() {
            self.body.clone()
        } else if self.body.is_empty() {
            self.footnotes.clone()
        } else {
            format!("{}\n{}", self.body, self.footnotes)
        }
    }
}

/// How an image resource is named in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageSource<'a> {
    DataUri,
    Relative(&'a str),
}

struct Note {
    id: String,
    number: usize,
    definition: FootnoteDefinition,
    refs: Vec<String>,
    body: Option<String>,
}

struct Builder<'a> {
    template: &'a Template,
    assets: &'a Assets,
    image_source: ImageSource<'a>,
    definitions: BTreeMap<String, FootnoteDefinition>,
    notes: Vec<Note>,
    note_index: BTreeMap<String, usize>,
    active: Vec<String>,
    warnings: Vec<String>,
    text_width: f64,
}

/// Renders the document body and its collected endnotes.
pub(crate) fn render(document: &Document, template: &Template, assets: &Assets) -> Content {
    render_with(document, template, assets, ImageSource::DataUri)
}

/// Renders the document body and its collected endnotes with a chosen image
/// source.
pub(crate) fn render_with(
    document: &Document,
    template: &Template,
    assets: &Assets,
    image_source: ImageSource<'_>,
) -> Content {
    let page = template.page();
    let dimensions = page.dimensions();
    let margins = page.margins();
    let text_width =
        (dimensions.width().points() - margins.left().points() - margins.right().points()).max(1.0);
    let mut builder = Builder {
        template,
        assets,
        image_source,
        definitions: BTreeMap::new(),
        notes: Vec::new(),
        note_index: BTreeMap::new(),
        active: Vec::new(),
        warnings: Vec::new(),
        text_width,
    };
    builder.collect_definitions(document.blocks());
    let body = builder.blocks_to_string(document.blocks(), 0, true);
    builder.render_notes();
    let footnotes = builder.footnotes();
    Content {
        body,
        footnotes,
        warnings: builder.warnings,
    }
}

impl Builder<'_> {
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
                Block::Heading(_)
                | Block::Paragraph(_)
                | Block::Image(_)
                | Block::NumberedParagraph(_)
                | Block::Table(_)
                | Block::CodeBlock(_)
                | Block::ThematicBreak
                | Block::PageBreak => {}
            }
        }
    }

    fn blocks_to_string(&mut self, blocks: &[Block], depth: usize, body: bool) -> String {
        let mut rendered = Vec::new();
        for block in blocks {
            let text = self.block(block, depth, body);
            if !text.is_empty() {
                rendered.push(text);
            }
        }
        rendered.join("\n")
    }

    fn block(&mut self, block: &Block, depth: usize, body: bool) -> String {
        match block {
            Block::Heading(heading) => {
                let level = heading.level().get();
                format!("<h{level}>{}</h{level}>", self.inlines(heading.content()))
            }
            Block::Paragraph(inlines) => self.paragraph(inlines, body, None),
            Block::Image(image) => self.image_block(image),
            Block::NumberedParagraph(paragraph) => {
                self.paragraph(paragraph.content(), body, Some(paragraph.number()))
            }
            Block::List(list) => self.list(list, depth),
            Block::Table(table) => self.table(table),
            Block::Quote(quote) => self.quote(quote, depth),
            Block::CodeBlock(code) => self.code_block(code),
            Block::ThematicBreak => "<hr class=\"horizontal-rule\" />".to_owned(),
            Block::PageBreak => "<div class=\"page-break\" aria-hidden=\"true\"></div>".to_owned(),
            Block::FootnoteDefinition(_) => String::new(),
        }
    }

    fn paragraph(&mut self, inlines: &[Inline], body: bool, number: Option<u64>) -> String {
        let content = self.inlines(inlines);
        match number {
            Some(number) => {
                let class = if body {
                    " class=\"body-paragraph numbered\""
                } else {
                    " class=\"numbered\""
                };
                format!("<p{class}><span class=\"number\">{number}.</span> {content}</p>")
            }
            None => {
                let class = if body {
                    " class=\"body-paragraph\""
                } else {
                    ""
                };
                format!("<p{class}>{content}</p>")
            }
        }
    }

    fn list(&mut self, list: &List, depth: usize) -> String {
        let ordered = matches!(list.start(), ListStart::Ordered(_));
        let tag = if ordered { "ol" } else { "ul" };
        let mut classes = vec![format!("list-depth-{depth}")];
        if !list.is_tight() {
            classes.push("list-loose".to_owned());
        }
        let task_style = self.template.formatting().list().task();
        if !ordered && list.is_task_list() && !task_style.bullet() {
            classes.push("task-list".to_owned());
        }
        let start = match list.start() {
            ListStart::Ordered(start) if start != 1 => format!(" start=\"{start}\""),
            _ => String::new(),
        };
        let mut output = format!("<{tag} class=\"{}\"{start}>\n", classes.join(" "));
        for item in list.items() {
            output.push_str("<li>");
            output.push_str(&self.item(item, depth));
            output.push_str("</li>\n");
        }
        output.push_str(&format!("</{tag}>"));
        output
    }

    fn item(&mut self, item: &ListItem, depth: usize) -> String {
        let marker = item.task().map(|state| self.task_marker(state));
        let gap = "\u{a0}".repeat(self.template.formatting().list().task().gap());
        let separator = marker.as_ref().map(|marker| format!("{marker}{gap}"));
        let blocks = item.blocks();
        let mut output = String::new();
        if let Some((Block::Paragraph(inlines), rest)) = blocks.split_first() {
            output.push_str("<p class=\"list-paragraph\">");
            if let Some(separator) = &separator {
                output.push_str(separator);
            }
            output.push_str(&self.inlines(inlines));
            output.push_str("</p>");
            if !rest.is_empty() {
                output.push('\n');
                output.push_str(&self.blocks_to_string(rest, depth.saturating_add(1), false));
            }
        } else {
            if let Some(separator) = &separator {
                output.push_str(separator);
            }
            output.push_str(&self.blocks_to_string(blocks, depth.saturating_add(1), false));
        }
        output
    }

    fn task_marker(&self, state: TaskState) -> String {
        let style = self.template.formatting().list().task();
        let marker = match state {
            TaskState::Done => style.done(),
            TaskState::Todo => style.todo(),
        };
        let class = match state {
            TaskState::Done => "task-done",
            TaskState::Todo => "task-todo",
        };
        format!(
            "<span class=\"{class}\">{}</span>",
            escape::text(marker.marker())
        )
    }

    fn table(&mut self, table: &Table) -> String {
        let mut output = String::from("<table>\n<thead>\n<tr>\n");
        for (cell, alignment) in table.header().iter().zip(table.alignments()) {
            output.push_str(&self.cell("th", cell, *alignment, true));
            output.push('\n');
        }
        output.push_str("</tr>\n</thead>\n<tbody>\n");
        for row in table.rows() {
            output.push_str("<tr>\n");
            for (cell, alignment) in row.iter().zip(table.alignments()) {
                output.push_str(&self.cell("td", cell, *alignment, false));
                output.push('\n');
            }
            output.push_str("</tr>\n");
        }
        output.push_str("</tbody>\n</table>");
        output
    }

    fn cell(
        &mut self,
        tag: &str,
        cell: &TableCell,
        alignment: ColumnAlignment,
        header: bool,
    ) -> String {
        let class = match alignment {
            ColumnAlignment::Default => "",
            ColumnAlignment::Left => " class=\"align-left\"",
            ColumnAlignment::Center => " class=\"align-center\"",
            ColumnAlignment::Right => " class=\"align-right\"",
        };
        let scope = if header { " scope=\"col\"" } else { "" };
        format!(
            "<{tag}{scope}{class}>{}</{tag}>",
            self.inlines(cell.content())
        )
    }

    fn quote(&mut self, quote: &Quote, depth: usize) -> String {
        let (tag, role, class) = match quote.kind() {
            QuoteKind::Plain => ("blockquote", "", "quote".to_owned()),
            QuoteKind::Alert(alert) => (
                "aside",
                " role=\"note\"",
                format!("quote alert alert-{}", alert_class(alert)),
            ),
        };
        let mut output = format!("<{tag} class=\"{class}\"{role}>\n");
        if let QuoteKind::Alert(alert) = quote.kind() {
            output.push_str(&format!(
                "<p class=\"alert-label\">[{}]</p>\n",
                escape::text(alert.label())
            ));
        }
        let content = self.blocks_to_string(quote.blocks(), depth, false);
        output.push_str(&content);
        if !content.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!("</{tag}>"));
        output
    }

    fn code_block(&mut self, code: &CodeBlock) -> String {
        let class = code
            .language()
            .and_then(|language| class_token(language.as_str()))
            .map(|token| format!(" class=\"language-{token}\""))
            .unwrap_or_default();
        format!(
            "<pre class=\"code-block\"><code{class}>{}</code></pre>",
            escape::text(code.text())
        )
    }

    fn inlines(&mut self, inlines: &[Inline]) -> String {
        let mut output = String::new();
        for inline in inlines {
            output.push_str(&self.inline(inline));
        }
        output
    }

    fn inline(&mut self, inline: &Inline) -> String {
        match inline {
            Inline::Text(text) => escape::text(text),
            Inline::Code(code) => {
                format!("<code class=\"inline-code\">{}</code>", escape::text(code))
            }
            Inline::Emphasis(content) => {
                format!("<em class=\"emphasis\">{}</em>", self.inlines(content))
            }
            Inline::Strong(content) => {
                format!(
                    "<strong class=\"strong\">{}</strong>",
                    self.inlines(content)
                )
            }
            Inline::Strikethrough(content) => {
                format!("<s class=\"strikethrough\">{}</s>", self.inlines(content))
            }
            Inline::Link {
                destination,
                title,
                content,
            } => self.link(destination, title.as_deref(), content),
            Inline::Image {
                destination,
                title,
                alternative,
            } => self.image(destination, title.as_deref(), alternative),
            Inline::SoftBreak => " ".to_owned(),
            Inline::HardBreak => "<br />".to_owned(),
            Inline::FootnoteReference(id) => self.note(id),
        }
    }

    fn link(
        &mut self,
        destination: &Destination,
        title: Option<&str>,
        content: &[Inline],
    ) -> String {
        let label = {
            let label = self.inlines(content);
            if label.is_empty() {
                escape::text(destination.as_str())
            } else {
                label
            }
        };
        if destination.as_str().is_empty() {
            return label;
        }
        if !destination.scheme().is_allowed() {
            self.warn(
                "a link destination uses an unsupported scheme; it was rendered as text".to_owned(),
            );
            return label;
        }
        let title = title
            .map(|title| format!(" title=\"{}\"", escape::attribute(title)))
            .unwrap_or_default();
        format!(
            "<a href=\"{}\"{title}>{label}</a>",
            escape::attribute(destination.as_str())
        )
    }

    fn image(
        &mut self,
        destination: &Destination,
        title: Option<&str>,
        alternative: &[Inline],
    ) -> String {
        let alternative = plain_text(alternative);
        let Some(asset) = self.assets.map().image(destination.as_str()) else {
            self.warn(format!(
                "image `{}` has no resolved asset; its alternative text was used",
                destination.as_str()
            ));
            return missing_image(&alternative);
        };
        let Some(bytes) = self.assets.files().get(asset) else {
            self.warn(format!(
                "image `{}` has bytes that were not loaded; its alternative text was used",
                destination.as_str()
            ));
            return missing_image(&alternative);
        };
        let Some(media) = media_type(asset) else {
            self.warn(format!(
                "image `{}` has an unsupported media type; its alternative text was used",
                destination.as_str()
            ));
            return missing_image(&alternative);
        };
        let source = match self.image_source {
            ImageSource::DataUri => data_uri(media, bytes),
            ImageSource::Relative(prefix) => {
                format!(
                    "{prefix}{}",
                    asset.rsplit_once('/').map_or(asset, |(_, name)| name)
                )
            }
        };
        let title = title
            .map(|title| format!(" title=\"{}\"", escape::attribute(title)))
            .unwrap_or_default();
        format!(
            "<img src=\"{}\" alt=\"{}\"{title} />",
            escape::attribute(&source),
            escape::attribute(&alternative)
        )
    }

    fn image_block(&mut self, image: &ImageBlock) -> String {
        let alternative = plain_text(image.alternative());
        let Some(asset) = self.assets.map().image(image.destination().as_str()) else {
            self.warn(format!(
                "image `{}` has no resolved asset; its alternative text was used",
                image.destination().as_str()
            ));
            return format!(
                "<p class=\"image-block\">[{}]</p>",
                escape::text(&alternative)
            );
        };
        let Some(bytes) = self.assets.files().get(asset) else {
            self.warn(format!(
                "image `{}` has bytes that were not loaded; its alternative text was used",
                image.destination().as_str()
            ));
            return format!(
                "<p class=\"image-block\">[{}]</p>",
                escape::text(&alternative)
            );
        };
        let Some(media) = media_type(asset) else {
            self.warn(format!(
                "image `{}` has an unsupported media type; its alternative text was used",
                image.destination().as_str()
            ));
            return format!(
                "<p class=\"image-block\">[{}]</p>",
                escape::text(&alternative)
            );
        };
        let source = match self.image_source {
            ImageSource::DataUri => data_uri(media, bytes),
            ImageSource::Relative(prefix) => {
                format!(
                    "{prefix}{}",
                    asset.rsplit_once('/').map_or(asset, |(_, name)| name)
                )
            }
        };
        let image_style = self.template.formatting().image();
        let padding = if image_style.background().is_some() || !image_style.border().is_empty() {
            image_style.border().padding().points()
        } else {
            0.0
        };
        let layout = image::layout(
            image::intrinsic(bytes),
            self.text_width,
            image.size(),
            image_style.max_width(),
            padding,
        );
        if layout.is_capped() {
            self.warn(format!(
                "image `{}` was reduced to the text width",
                image.destination().as_str()
            ));
        }
        let alignment = image
            .alignment()
            .unwrap_or_else(|| self.template.formatting().image().alignment());
        let class = match alignment {
            ImageAlignment::Left => "align-left",
            ImageAlignment::Center => "align-center",
            ImageAlignment::Right => "align-right",
        };
        let width = match image.size() {
            ImageSize::Auto => String::new(),
            ImageSize::Width(ImageDimension::Percent(percent))
            | ImageSize::Height(ImageDimension::Percent(percent)) => {
                format!(" style=\"width: {percent:.2}%;\"")
            }
            ImageSize::Width(_) | ImageSize::Height(_) => {
                format!(" style=\"width: {:.2}pt;\"", layout.width())
            }
        };
        let title = image
            .title()
            .map(|title| format!(" title=\"{}\"", escape::attribute(title)))
            .unwrap_or_default();
        format!(
            "<figure class=\"image-block {class}\"><span class=\"image-frame\"><img src=\"{}\" alt=\"{}\"{title}{width} /></span></figure>",
            escape::attribute(&source),
            escape::attribute(&alternative)
        )
    }

    fn note(&mut self, id: &FootnoteId) -> String {
        let key = id.as_str();
        if self.active.iter().any(|active| active == key) {
            return escape::text(&format!("[^{key}]"));
        }
        let Some(definition) = self.definitions.get(key).cloned() else {
            self.warn("a footnote reference has no definition; it was rendered as text".to_owned());
            return escape::text(&format!("[^{key}]"));
        };
        let index = match self.note_index.get(key).copied() {
            Some(index) => index,
            None => {
                let index = self.notes.len();
                self.notes.push(Note {
                    id: key.to_owned(),
                    number: index.saturating_add(1),
                    definition,
                    refs: Vec::new(),
                    body: None,
                });
                self.note_index.insert(key.to_owned(), index);
                index
            }
        };
        let number = self.notes[index].number;
        let reference = self.notes[index].refs.len().saturating_add(1);
        let dom_id = if reference == 1 {
            format!("fnref-{number}")
        } else {
            format!("fnref-{number}-{reference}")
        };
        self.notes[index].refs.push(dom_id.clone());
        format!(
            "<sup><a href=\"#fn-{number}\" id=\"{dom_id}\" role=\"doc-noteref\">{number}</a></sup>"
        )
    }

    fn render_notes(&mut self) {
        let mut index = 0;
        while index < self.notes.len() {
            let id = self.notes[index].id.clone();
            let definition = self.notes[index].definition.clone();
            self.active.push(id);
            let body = self.blocks_to_string(definition.blocks(), 0, false);
            self.active.pop();
            self.notes[index].body = Some(body);
            index = index.saturating_add(1);
        }
    }

    fn footnotes(&self) -> String {
        if self.notes.is_empty() {
            return String::new();
        }
        let mut output =
            String::from("<section class=\"footnotes\" role=\"doc-endnotes\">\n<ol>\n");
        for note in &self.notes {
            output.push_str(&format!(
                "<li id=\"fn-{}\" role=\"doc-endnote\">\n",
                note.number
            ));
            if let Some(body) = &note.body {
                output.push_str(body);
                output.push('\n');
            }
            if !note.refs.is_empty() {
                output.push_str("<p class=\"footnote-backrefs\">");
                for (index, reference) in note.refs.iter().enumerate() {
                    if index > 0 {
                        output.push(' ');
                    }
                    output.push_str(&format!(
                        "<a href=\"#{}\" role=\"doc-backlink\">↩</a>",
                        escape::attribute(reference)
                    ));
                }
                output.push_str("</p>\n");
            }
            output.push_str("</li>\n");
        }
        output.push_str("</ol>\n</section>");
        output
    }

    fn warn(&mut self, message: String) {
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }
}

pub(crate) fn plain_text(inlines: &[Inline]) -> String {
    let mut output = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) => output.push_str(text),
            Inline::Emphasis(content)
            | Inline::Strong(content)
            | Inline::Strikethrough(content) => output.push_str(&plain_text(content)),
            Inline::Link {
                destination,
                content,
                ..
            } => {
                let label = plain_text(content);
                if label.is_empty() {
                    output.push_str(destination.as_str());
                } else {
                    output.push_str(&label);
                }
            }
            Inline::Image { alternative, .. } => output.push_str(&plain_text(alternative)),
            Inline::SoftBreak | Inline::HardBreak => output.push(' '),
            Inline::FootnoteReference(id) => {
                output.push_str(&format!("[^{}]", id.as_str()));
            }
        }
    }
    output
}

fn missing_image(alternative: &str) -> String {
    format!(
        "<span class=\"missing-image\">[{}]</span>",
        escape::text(alternative)
    )
}

fn class_token(value: &str) -> Option<String> {
    let token = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '+')
        })
        .collect::<String>();
    if token.is_empty() { None } else { Some(token) }
}

fn alert_class(alert: Alert) -> &'static str {
    match alert {
        Alert::Note => "note",
        Alert::Tip => "tip",
        Alert::Important => "important",
        Alert::Warning => "warning",
        Alert::Caution => "caution",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        CodeLanguage, Document, Heading, HeadingLevel, List, ListItem, Metadata, NumberedParagraph,
        TableCell,
    };
    use crate::markdown;
    use crate::template::Template;

    fn embedded() -> Template {
        Template::embedded().expect("the embedded template must be valid")
    }

    fn render_source(source: &str) -> Content {
        let document = markdown::parse(source).expect("the document must parse");
        render(&document, &embedded(), &Assets::default())
    }

    #[test]
    fn headings_and_paragraphs_are_semantic() {
        let content = render_source("# Título\n\nTexto.\n");

        assert!(
            content.full().contains("<h1>Título</h1>"),
            "{}",
            content.full()
        );
        assert!(
            content
                .full()
                .contains("<p class=\"body-paragraph\">Texto.</p>"),
            "{}",
            content.full()
        );
    }

    #[test]
    fn an_empty_document_has_no_body_blocks() {
        let document = Document::new(Metadata::default(), Vec::new());
        let content = render(&document, &embedded(), &Assets::default());

        assert!(content.full().is_empty());
        assert!(content.warnings.is_empty());
    }

    #[test]
    fn numbered_paragraphs_keep_their_literal_number() {
        let document = Document::new(
            Metadata::default(),
            vec![Block::NumberedParagraph(NumberedParagraph::new(
                3,
                vec![Inline::Text("Cláusula.".to_owned())],
            ))],
        );
        let content = render(&document, &embedded(), &Assets::default());

        assert!(
            content
                .full()
                .contains("<span class=\"number\">3.</span> Cláusula."),
            "{}",
            content.full()
        );
    }

    #[test]
    fn lists_tasks_and_alerts_keep_their_semantics() {
        let content = render_source(
            "- [x] feito\n- [ ] aberto\n\n> [!WARNING]\n> Cuidado.\n\n```rust\nlet x = 1;\n```\n",
        );

        assert!(
            content
                .full()
                .contains("<ul class=\"list-depth-0 task-list\">"),
            "{}",
            content.full()
        );
        assert!(
            content
                .full()
                .contains("<span class=\"task-done\">[x]</span>"),
            "{}",
            content.full()
        );
        assert!(
            content
                .full()
                .contains("<aside class=\"quote alert alert-warning\" role=\"note\">"),
            "{}",
            content.full()
        );
        assert!(content.full().contains("[!WARNING]"), "{}", content.full());
        assert!(
            content
                .full()
                .contains("<code class=\"language-rust\">let x = 1;"),
            "{}",
            content.full()
        );
    }

    #[test]
    fn a_table_has_a_header_and_column_alignment() {
        let content = render_source("| A | B |\n|:--|--:|\n| 1 | 2 |\n");

        assert!(content.full().contains("<thead>"), "{}", content.full());
        assert!(
            content
                .full()
                .contains("<th scope=\"col\" class=\"align-left\">A</th>"),
            "{}",
            content.full()
        );
        assert!(
            content.full().contains("<td class=\"align-right\">2</td>"),
            "{}",
            content.full()
        );
    }

    #[test]
    fn footnotes_are_numbered_in_reference_order_and_linked() {
        let content = render_source("Texto[^b] e[^a].\n\n[^a]: Nota A.\n[^b]: Nota B.\n");

        assert!(
            content
                .full()
                .contains("<section class=\"footnotes\" role=\"doc-endnotes\">"),
            "{}",
            content.full()
        );
        assert!(content.full().contains("id=\"fn-1\""), "{}", content.full());
        assert!(content.full().contains("Nota B."), "{}", content.full());
        assert!(
            content
                .full()
                .contains("<a href=\"#fnref-1\" role=\"doc-backlink\">↩</a>"),
            "{}",
            content.full()
        );
    }

    #[test]
    fn an_unsafe_link_is_text_with_a_warning() {
        let content = render_source("[clique](javascript:alert(1))\n");

        assert!(content.full().contains("<p"), "{}", content.full());
        assert!(content.full().contains("clique"), "{}", content.full());
        assert!(!content.full().contains("href"), "{}", content.full());
        assert_eq!(content.warnings.len(), 1);
    }

    #[test]
    fn a_missing_image_falls_back_to_its_alternative_text() {
        let document = Document::new(
            Metadata::default(),
            vec![Block::Paragraph(vec![Inline::Image {
                destination: Destination::new("missing.png"),
                title: None,
                alternative: vec![Inline::Text("Logo".to_owned())],
            }])],
        );
        let content = render(&document, &embedded(), &Assets::default());

        assert!(content.full().contains("[Logo]"), "{}", content.full());
        assert_eq!(content.warnings.len(), 1);
    }

    #[test]
    fn a_heading_level_and_a_code_language_are_validated() {
        let document = Document::new(
            Metadata::default(),
            vec![
                Block::Heading(Heading::new(
                    HeadingLevel::new(2).expect("a level"),
                    vec![Inline::Text("Seção".to_owned())],
                )),
                Block::CodeBlock(CodeBlock::new(
                    CodeLanguage::parse("c++"),
                    "int main();\n".to_owned(),
                )),
            ],
        );
        let content = render(&document, &embedded(), &Assets::default());

        assert!(
            content.full().contains("<h2>Seção</h2>"),
            "{}",
            content.full()
        );
        assert!(
            content.full().contains("class=\"language-c++\""),
            "{}",
            content.full()
        );
    }

    #[test]
    fn a_nested_list_uses_the_next_depth() {
        let document = Document::new(
            Metadata::default(),
            vec![Block::List(List::new(
                ListStart::Unordered,
                vec![ListItem::new(
                    None,
                    vec![
                        Block::Paragraph(vec![Inline::Text("um".to_owned())]),
                        Block::List(List::new(
                            ListStart::Unordered,
                            vec![ListItem::new(
                                None,
                                vec![Block::Paragraph(vec![Inline::Text("dois".to_owned())])],
                            )],
                            true,
                        )),
                    ],
                )],
                true,
            ))],
        );
        let content = render(&document, &embedded(), &Assets::default());

        assert!(
            content.full().contains("list-depth-0"),
            "{}",
            content.full()
        );
        assert!(
            content.full().contains("list-depth-1"),
            "{}",
            content.full()
        );
        assert!(
            content
                .full()
                .contains("<p class=\"list-paragraph\">um</p>"),
            "{}",
            content.full()
        );
    }

    #[test]
    fn a_table_cell_keeps_its_alignment() {
        let table = Table::new(
            vec![ColumnAlignment::Center],
            vec![TableCell::new(vec![Inline::Text("A".to_owned())])],
            Vec::new(),
        )
        .expect("a valid table");
        let document = Document::new(Metadata::default(), vec![Block::Table(table)]);
        let content = render(&document, &embedded(), &Assets::default());

        assert!(
            content.full().contains("class=\"align-center\""),
            "{}",
            content.full()
        );
    }
}
