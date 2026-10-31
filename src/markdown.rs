//! Parses CommonMark plus the project extensions into the document model.
//!
//! The accepted extensions are the ones the Zed preview renders: tables, task
//! lists, strikethrough, footnotes, GFM alert block quotes and YAML frontmatter.
//! Raw HTML is outside the contract and is reported as an error instead of being
//! silently dropped.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel as SourceHeadingLevel,
    MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use serde::Deserialize;

use crate::document::{
    Alert, Author, Block, CodeBlock, CodeLanguage, ColumnAlignment, Destination, Document,
    DocumentDate, FootnoteDefinition, FootnoteId, Heading, HeadingLevel, Identifier, Inline,
    Keyword, Language, List, ListItem, ListStart, Metadata, MetadataError, NumberedParagraph,
    Quote, QuoteKind, Subject, Table, TableCell, TableError, TaskState, Title,
};

/// The deepest tag nesting accepted. A deeper document is rejected instead of
/// overflowing the stack of the recursive descent.
const MAX_NESTING: usize = 128;

/// Parses a Markdown source into the document model.
pub fn parse(source: &str) -> Result<Document, MarkdownError> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let events = Parser::new_ext(source, options).collect();
    DocumentParser::new(events).parse_document()
}

/// Why a Markdown source could not be parsed.
#[derive(Debug)]
pub enum MarkdownError {
    HtmlNotSupported,
    NestingTooDeep { limit: usize },
    Malformed { message: String },
    Frontmatter { message: String },
    Metadata(MetadataError),
    Table(TableError),
}

impl std::fmt::Display for MarkdownError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HtmlNotSupported => formatter.write_str("raw HTML is not supported"),
            Self::NestingTooDeep { limit } => {
                write!(
                    formatter,
                    "the document is nested deeper than {limit} levels"
                )
            }
            Self::Malformed { message } => write!(formatter, "malformed Markdown: {message}"),
            Self::Frontmatter { message } => write!(formatter, "invalid frontmatter: {message}"),
            Self::Metadata(error) => write!(formatter, "invalid frontmatter: {error}"),
            Self::Table(error) => write!(formatter, "invalid table: {error}"),
        }
    }
}

impl std::error::Error for MarkdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Metadata(error) => Some(error),
            Self::Table(error) => Some(error),
            Self::HtmlNotSupported
            | Self::NestingTooDeep { .. }
            | Self::Malformed { .. }
            | Self::Frontmatter { .. } => None,
        }
    }
}

struct DocumentParser<'source> {
    events: Vec<Event<'source>>,
    index: usize,
    metadata: Option<Metadata>,
}

struct ParsedList {
    items: Vec<ListItem>,
    tight: bool,
}

struct ParsedItem {
    item: ListItem,
    tight: bool,
}

impl<'source> DocumentParser<'source> {
    fn new(events: Vec<Event<'source>>) -> Self {
        Self {
            events,
            index: 0,
            metadata: None,
        }
    }

    fn parse_document(mut self) -> Result<Document, MarkdownError> {
        let mut blocks = Vec::new();
        while let Some(event) = self.next() {
            blocks.extend(self.parse_block_event(event, 0)?);
        }
        Ok(Document::new(self.metadata.unwrap_or_default(), blocks))
    }

    fn next(&mut self) -> Option<Event<'source>> {
        let event = self.events.get(self.index).cloned();
        if event.is_some() {
            self.index += 1;
        }
        event
    }

    fn peek(&self) -> Option<&Event<'source>> {
        self.events.get(self.index)
    }

    fn parse_block_event(
        &mut self,
        event: Event<'source>,
        depth: usize,
    ) -> Result<Vec<Block>, MarkdownError> {
        match event {
            Event::Start(Tag::MetadataBlock(kind)) => {
                self.parse_frontmatter(kind)?;
                Ok(Vec::new())
            }
            Event::Start(tag) => self.parse_start_tag(tag, depth),
            Event::Rule => Ok(vec![Block::ThematicBreak]),
            Event::Html(_) | Event::InlineHtml(_) => Err(MarkdownError::HtmlNotSupported),
            other => Err(MarkdownError::Malformed {
                message: format!("unexpected event at block level: {other:?}"),
            }),
        }
    }

    fn parse_start_tag(
        &mut self,
        tag: Tag<'source>,
        depth: usize,
    ) -> Result<Vec<Block>, MarkdownError> {
        if depth >= MAX_NESTING {
            return Err(MarkdownError::NestingTooDeep { limit: MAX_NESTING });
        }
        match tag {
            Tag::Paragraph => {
                let content = self.parse_inlines(TagEnd::Paragraph, depth)?;
                Ok(paragraph_blocks(content))
            }
            Tag::Heading { level: source, .. } => {
                let end = TagEnd::Heading(source);
                let level = heading_level(source)?;
                let content = self.parse_inlines(end, depth)?;
                Ok(vec![Block::Heading(Heading::new(level, content))])
            }
            Tag::BlockQuote(kind) => {
                let end = TagEnd::BlockQuote(kind);
                let quote_kind = quote_kind(kind);
                let blocks = self.parse_blocks(end, depth + 1)?;
                Ok(vec![Block::Quote(Quote::new(quote_kind, blocks))])
            }
            Tag::List(start) => {
                let parsed = self.parse_items(start, depth + 1)?;
                if let Some(number) = start
                    && let Some(paragraphs) = numbered_paragraphs(number, &parsed)
                {
                    return Ok(paragraphs);
                }
                let start = match start {
                    Some(number) => ListStart::Ordered(number),
                    None => ListStart::Unordered,
                };
                Ok(vec![Block::List(List::new(
                    start,
                    parsed.items,
                    parsed.tight,
                ))])
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(info) => CodeLanguage::parse(&info),
                };
                let text = self.collect_text(TagEnd::CodeBlock, "code block")?;
                Ok(vec![Block::CodeBlock(CodeBlock::new(language, text))])
            }
            Tag::Table(alignments) => {
                let table = self.parse_table(alignments, depth + 1)?;
                Ok(vec![Block::Table(table)])
            }
            Tag::FootnoteDefinition(name) => {
                let id = FootnoteId::parse(&name)
                    .ok_or_else(|| malformed("a footnote definition has an empty label"))?;
                let blocks = self.parse_blocks(TagEnd::FootnoteDefinition, depth + 1)?;
                Ok(vec![Block::FootnoteDefinition(FootnoteDefinition::new(
                    id, blocks,
                ))])
            }
            Tag::HtmlBlock => Err(MarkdownError::HtmlNotSupported),
            Tag::Item => Err(malformed("a list item appeared outside a list")),
            Tag::MetadataBlock(_) => Err(malformed(
                "frontmatter is only allowed at the top level of the document",
            )),
            other => Err(malformed(format!("unsupported block tag: {other:?}"))),
        }
    }

    fn parse_blocks(&mut self, end: TagEnd, depth: usize) -> Result<Vec<Block>, MarkdownError> {
        let mut blocks = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(Event::End(actual)) if actual == end => {
                    self.index += 1;
                    return Ok(blocks);
                }
                Some(_) => {
                    let event = self
                        .next()
                        .ok_or_else(|| malformed("unexpected end of the document"))?;
                    blocks.extend(self.parse_block_event(event, depth)?);
                }
                None => {
                    return Err(malformed(format!(
                        "a block started without its closing tag {end:?}"
                    )));
                }
            }
        }
    }

    fn parse_items(
        &mut self,
        start: Option<u64>,
        depth: usize,
    ) -> Result<ParsedList, MarkdownError> {
        let end = TagEnd::List(start.is_some());
        let mut items = Vec::new();
        let mut tight = true;
        loop {
            match self.peek().cloned() {
                Some(Event::Start(Tag::Item)) => {
                    self.index += 1;
                    let parsed = self.parse_item(depth)?;
                    if !parsed.tight {
                        tight = false;
                    }
                    items.push(parsed.item);
                }
                Some(Event::End(actual)) if actual == end => {
                    self.index += 1;
                    return Ok(ParsedList { items, tight });
                }
                Some(other) => {
                    return Err(malformed(format!("unexpected event in a list: {other:?}")));
                }
                None => return Err(malformed("a list started without its closing tag")),
            }
        }
    }

    fn parse_item(&mut self, depth: usize) -> Result<ParsedItem, MarkdownError> {
        let mut blocks = Vec::new();
        let mut task = None;
        let mut tight = true;
        loop {
            match self.peek().cloned() {
                Some(Event::End(TagEnd::Item)) => {
                    self.index += 1;
                    return Ok(ParsedItem {
                        item: ListItem::new(task, blocks),
                        tight,
                    });
                }
                Some(Event::TaskListMarker(checked)) if blocks.is_empty() && task.is_none() => {
                    self.index += 1;
                    task = Some(task_state(checked));
                }
                Some(Event::Start(Tag::Paragraph)) => {
                    self.index += 1;
                    tight = false;
                    if blocks.is_empty() && task.is_none() {
                        task = self.take_task_marker();
                    }
                    let content = self.parse_inlines(TagEnd::Paragraph, depth)?;
                    blocks.extend(paragraph_blocks(content));
                }
                Some(event) if is_inline_event(&event) => {
                    let content = self.parse_inline_run(None, depth)?;
                    blocks.push(Block::Paragraph(content));
                }
                Some(Event::Start(tag)) => {
                    self.index += 1;
                    blocks.extend(self.parse_start_tag(tag, depth)?);
                }
                Some(Event::Rule) => {
                    self.index += 1;
                    blocks.push(Block::ThematicBreak);
                }
                Some(Event::Html(_)) | Some(Event::InlineHtml(_)) => {
                    return Err(MarkdownError::HtmlNotSupported);
                }
                Some(other) => {
                    return Err(malformed(format!(
                        "unexpected event in a list item: {other:?}"
                    )));
                }
                None => return Err(malformed("a list item started without its closing tag")),
            }
        }
    }

    /// Consumes a task list marker when the item starts with one.
    fn take_task_marker(&mut self) -> Option<TaskState> {
        match self.peek().cloned() {
            Some(Event::TaskListMarker(checked)) => {
                self.index += 1;
                Some(task_state(checked))
            }
            _ => None,
        }
    }

    fn parse_table(
        &mut self,
        alignments: Vec<Alignment>,
        depth: usize,
    ) -> Result<Table, MarkdownError> {
        let mut header = None;
        let mut rows = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(Event::Start(Tag::TableHead)) => {
                    self.index += 1;
                    header = Some(self.parse_cells(TagEnd::TableHead, depth)?);
                }
                Some(Event::Start(Tag::TableRow)) => {
                    self.index += 1;
                    rows.push(self.parse_cells(TagEnd::TableRow, depth)?);
                }
                Some(Event::End(TagEnd::Table)) => {
                    self.index += 1;
                    break;
                }
                Some(other) => {
                    return Err(malformed(format!("unexpected event in a table: {other:?}")));
                }
                None => return Err(malformed("a table started without its closing tag")),
            }
        }
        let header = header.ok_or_else(|| malformed("a table has no header row"))?;
        let alignments = alignments
            .into_iter()
            .map(column_alignment)
            .collect::<Vec<_>>();
        Table::new(alignments, header, rows).map_err(MarkdownError::Table)
    }

    fn parse_cells(&mut self, end: TagEnd, depth: usize) -> Result<Vec<TableCell>, MarkdownError> {
        let mut cells = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(Event::Start(Tag::TableCell)) => {
                    self.index += 1;
                    let content = self.parse_inlines(TagEnd::TableCell, depth)?;
                    cells.push(TableCell::new(content));
                }
                Some(Event::End(actual)) if actual == end => {
                    self.index += 1;
                    return Ok(cells);
                }
                Some(other) => {
                    return Err(malformed(format!(
                        "unexpected event in a table row: {other:?}"
                    )));
                }
                None => return Err(malformed("a table row started without its closing tag")),
            }
        }
    }

    fn parse_inlines(&mut self, end: TagEnd, depth: usize) -> Result<Vec<Inline>, MarkdownError> {
        self.parse_inline_run(Some(end), depth)
    }

    /// Parses inline events until `end`, or until the first block level event
    /// when `end` is `None`. Tight list items have no paragraph tags, so their
    /// first inline run is parsed this way.
    fn parse_inline_run(
        &mut self,
        end: Option<TagEnd>,
        depth: usize,
    ) -> Result<Vec<Inline>, MarkdownError> {
        if depth >= MAX_NESTING {
            return Err(MarkdownError::NestingTooDeep { limit: MAX_NESTING });
        }
        let mut inlines = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(Event::End(actual)) if end == Some(actual) => {
                    self.index += 1;
                    return Ok(inlines);
                }
                Some(event) if is_inline_event(&event) => {
                    self.index += 1;
                    self.push_inline(event, depth, &mut inlines)?;
                }
                Some(Event::Html(_)) | Some(Event::InlineHtml(_)) => {
                    return Err(MarkdownError::HtmlNotSupported);
                }
                Some(_) if end.is_none() => return Ok(inlines),
                Some(other) => {
                    return Err(malformed(format!(
                        "unexpected event in inline content: {other:?}"
                    )));
                }
                None if end.is_none() => return Ok(inlines),
                None => {
                    return Err(malformed("inline content started without its closing tag"));
                }
            }
        }
    }

    fn push_inline(
        &mut self,
        event: Event<'source>,
        depth: usize,
        inlines: &mut Vec<Inline>,
    ) -> Result<(), MarkdownError> {
        match event {
            Event::Text(text) => inlines.push(Inline::Text(text.into_string())),
            Event::Code(code) => inlines.push(Inline::Code(code.into_string())),
            Event::SoftBreak => inlines.push(Inline::SoftBreak),
            Event::HardBreak => inlines.push(Inline::HardBreak),
            Event::FootnoteReference(name) => {
                let id = FootnoteId::parse(&name)
                    .ok_or_else(|| malformed("a footnote reference has an empty label"))?;
                inlines.push(Inline::FootnoteReference(id));
            }
            Event::Start(Tag::Emphasis) => {
                let content = self.parse_inlines(TagEnd::Emphasis, depth + 1)?;
                inlines.push(Inline::Emphasis(content));
            }
            Event::Start(Tag::Strong) => {
                let content = self.parse_inlines(TagEnd::Strong, depth + 1)?;
                inlines.push(Inline::Strong(content));
            }
            Event::Start(Tag::Strikethrough) => {
                let content = self.parse_inlines(TagEnd::Strikethrough, depth + 1)?;
                inlines.push(Inline::Strikethrough(content));
            }
            Event::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                let content = self.parse_inlines(TagEnd::Link, depth + 1)?;
                inlines.push(Inline::Link {
                    destination: Destination::new(&dest_url),
                    title: optional_text(&title),
                    content,
                });
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let alternative = self.parse_inlines(TagEnd::Image, depth + 1)?;
                inlines.push(Inline::Image {
                    destination: Destination::new(&dest_url),
                    title: optional_text(&title),
                    alternative,
                });
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                return Err(MarkdownError::HtmlNotSupported);
            }
            Event::TaskListMarker(_) => {
                return Err(malformed("a task list marker appeared outside a list item"));
            }
            other => {
                return Err(malformed(format!(
                    "unexpected event in inline content: {other:?}"
                )));
            }
        }
        Ok(())
    }

    fn collect_text(&mut self, end: TagEnd, context: &str) -> Result<String, MarkdownError> {
        let mut text = String::new();
        loop {
            match self.next() {
                Some(Event::Text(chunk)) => text.push_str(&chunk),
                Some(Event::End(actual)) if actual == end => return Ok(text),
                Some(other) => {
                    return Err(malformed(format!(
                        "unexpected event in a {context}: {other:?}"
                    )));
                }
                None => {
                    return Err(malformed(format!(
                        "a {context} started without its closing tag"
                    )));
                }
            }
        }
    }

    fn parse_frontmatter(&mut self, kind: MetadataBlockKind) -> Result<(), MarkdownError> {
        if kind != MetadataBlockKind::YamlStyle {
            return Err(MarkdownError::Frontmatter {
                message: "only YAML frontmatter is supported".to_owned(),
            });
        }
        if self.metadata.is_some() {
            return Err(MarkdownError::Frontmatter {
                message: "the document has more than one frontmatter block".to_owned(),
            });
        }
        let end = TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle);
        let yaml = self.collect_text(end, "frontmatter block")?;
        self.metadata = Some(frontmatter_metadata(&yaml)?);
        Ok(())
    }
}

/// A standalone marker paragraph is a page break or a horizontal rule. The
/// markers are ordinary text to CommonMark and to the Zed preview, so they
/// survive the parser without raw HTML or a backslash escape.
fn paragraph_blocks(content: Vec<Inline>) -> Vec<Block> {
    match paragraph_marker(&content) {
        Some(ParagraphMarker::PageBreak) => vec![Block::PageBreak],
        Some(ParagraphMarker::HorizontalRule) => vec![Block::ThematicBreak],
        None => vec![Block::Paragraph(content)],
    }
}

/// A marker that occupies a paragraph by itself.
enum ParagraphMarker {
    PageBreak,
    HorizontalRule,
}

fn paragraph_marker(content: &[Inline]) -> Option<ParagraphMarker> {
    let [Inline::Text(text)] = content else {
        return None;
    };
    match text.trim() {
        "-pagebreak-" | "-!-" => Some(ParagraphMarker::PageBreak),
        "-horizontal-" => Some(ParagraphMarker::HorizontalRule),
        _ => None,
    }
}

/// A loose ordered list whose items are one paragraph each is a sequence of
/// numbered paragraphs. This keeps legal numbering such as `1.` as text,
/// without an escape and without renumbering it as a list.
fn numbered_paragraphs(number: u64, list: &ParsedList) -> Option<Vec<Block>> {
    if list.items.is_empty() || (list.items.len() > 1 && list.tight) {
        return None;
    }
    let mut paragraphs = Vec::with_capacity(list.items.len());
    for (index, item) in list.items.iter().enumerate() {
        if item.task().is_some() {
            return None;
        }
        let [Block::Paragraph(inlines)] = item.blocks() else {
            return None;
        };
        let offset = u64::try_from(index).ok()?;
        let paragraph_number = number.checked_add(offset)?;
        paragraphs.push(Block::NumberedParagraph(NumberedParagraph::new(
            paragraph_number,
            inlines.clone(),
        )));
    }
    Some(paragraphs)
}

fn heading_level(level: SourceHeadingLevel) -> Result<HeadingLevel, MarkdownError> {
    let value = match level {
        SourceHeadingLevel::H1 => 1,
        SourceHeadingLevel::H2 => 2,
        SourceHeadingLevel::H3 => 3,
        SourceHeadingLevel::H4 => 4,
        SourceHeadingLevel::H5 => 5,
        SourceHeadingLevel::H6 => 6,
    };
    HeadingLevel::new(value).ok_or_else(|| malformed("a heading level is out of range"))
}

fn is_inline_event(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::Text(_)
            | Event::Code(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::FootnoteReference(_)
            | Event::Start(
                Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            )
    )
}

fn quote_kind(kind: Option<BlockQuoteKind>) -> QuoteKind {
    match kind {
        None => QuoteKind::Plain,
        Some(source) => QuoteKind::Alert(match source {
            BlockQuoteKind::Note => Alert::Note,
            BlockQuoteKind::Tip => Alert::Tip,
            BlockQuoteKind::Important => Alert::Important,
            BlockQuoteKind::Warning => Alert::Warning,
            BlockQuoteKind::Caution => Alert::Caution,
        }),
    }
}

fn column_alignment(alignment: Alignment) -> ColumnAlignment {
    match alignment {
        Alignment::None => ColumnAlignment::Default,
        Alignment::Left => ColumnAlignment::Left,
        Alignment::Center => ColumnAlignment::Center,
        Alignment::Right => ColumnAlignment::Right,
    }
}

fn task_state(checked: bool) -> TaskState {
    if checked {
        TaskState::Done
    } else {
        TaskState::Todo
    }
}

fn optional_text(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn malformed(message: impl Into<String>) -> MarkdownError {
    MarkdownError::Malformed {
        message: message.into(),
    }
}

/// The fields the frontmatter contract accepts.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frontmatter {
    title: Option<String>,
    author: Option<String>,
    language: Option<String>,
    date: Option<String>,
    subject: Option<String>,
    keywords: Option<Vec<String>>,
    identifier: Option<String>,
}

fn frontmatter_metadata(yaml: &str) -> Result<Metadata, MarkdownError> {
    if yaml.trim().is_empty() {
        return Ok(Metadata::default());
    }
    let raw: Frontmatter =
        serde_yaml::from_str(yaml).map_err(|error| MarkdownError::Frontmatter {
            message: error.to_string(),
        })?;
    for (value, field) in [
        (&raw.title, "title"),
        (&raw.author, "author"),
        (&raw.language, "language"),
        (&raw.date, "date"),
        (&raw.subject, "subject"),
        (&raw.identifier, "identifier"),
    ] {
        if let Some(text) = value {
            reject_confidential(text, field)?;
        }
    }
    if let Some(keywords) = &raw.keywords {
        for keyword in keywords {
            reject_confidential(keyword, "keywords")?;
        }
    }
    let title = raw
        .title
        .map(|text| Title::parse(&text))
        .transpose()
        .map_err(MarkdownError::Metadata)?;
    let author = raw
        .author
        .map(|text| Author::parse(&text))
        .transpose()
        .map_err(MarkdownError::Metadata)?;
    let language = raw
        .language
        .map(|text| Language::parse(&text))
        .transpose()
        .map_err(MarkdownError::Metadata)?;
    let date = raw
        .date
        .map(|text| DocumentDate::parse(&text))
        .transpose()
        .map_err(MarkdownError::Metadata)?;
    let subject = raw
        .subject
        .map(|text| Subject::parse(&text))
        .transpose()
        .map_err(MarkdownError::Metadata)?;
    let keywords = raw
        .keywords
        .unwrap_or_default()
        .iter()
        .map(|text| Keyword::parse(text))
        .collect::<Result<Vec<_>, _>>()
        .map_err(MarkdownError::Metadata)?;
    let identifier = raw
        .identifier
        .map(|text| Identifier::parse(&text))
        .transpose()
        .map_err(MarkdownError::Metadata)?;
    let metadata = Metadata::new(title, author, language);
    let metadata = match date {
        Some(date) => metadata.with_date(date),
        None => metadata,
    };
    let metadata = match subject {
        Some(subject) => metadata.with_subject(subject),
        None => metadata,
    };
    let metadata = match identifier {
        Some(identifier) => metadata.with_identifier(identifier),
        None => metadata,
    };
    Ok(metadata.with_keywords(keywords))
}

/// Confidential fields belong to the body, never to metadata that reaches the
/// PDF package.
fn reject_confidential(text: &str, field: &str) -> Result<(), MarkdownError> {
    match crate::confidential::first_field(text) {
        Some(name) => Err(MarkdownError::Frontmatter {
            message: format!(
                "confidential field `{{{name}}}` is not allowed in frontmatter `{field}`"
            ),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_text(inlines: &[Inline]) -> String {
        inlines
            .iter()
            .map(|inline| match inline {
                Inline::Text(text) | Inline::Code(text) => text.clone(),
                Inline::Emphasis(content)
                | Inline::Strong(content)
                | Inline::Strikethrough(content) => plain_text(content),
                Inline::Link { content, .. } => plain_text(content),
                Inline::Image { alternative, .. } => plain_text(alternative),
                Inline::SoftBreak | Inline::HardBreak => " ".to_owned(),
                Inline::FootnoteReference(id) => format!("[^{}]", id.as_str()),
            })
            .collect()
    }

    fn first_block(document: &Document) -> &Block {
        document.blocks().first().expect("a first block")
    }

    #[test]
    fn empty_input_is_an_empty_document() {
        let document = parse("").expect("an empty document");
        assert_eq!(document.block_count(), 0);
        assert_eq!(document.metadata(), &Metadata::default());
    }

    #[test]
    fn frontmatter_fills_the_metadata() {
        let document =
            parse("---\ntitle: Contrato\nauthor: Pageformer\nlanguage: pt-BR\n---\n\nTexto.\n")
                .expect("valid frontmatter");
        let metadata = document.metadata();
        assert_eq!(metadata.title().map(Title::as_str), Some("Contrato"));
        assert_eq!(metadata.author().map(Author::as_str), Some("Pageformer"));
        assert_eq!(metadata.language().map(Language::as_str), Some("pt-BR"));
    }

    #[test]
    fn empty_frontmatter_is_allowed() {
        let document = parse("---\n---\n\nTexto.\n").expect("empty frontmatter");
        assert_eq!(document.metadata(), &Metadata::default());
    }

    #[test]
    fn frontmatter_fills_the_remaining_metadata() {
        let document = parse(
            "---\ndate: 2026-03-05\nsubject: Contrato\nkeywords:\n  - legal\n  - contrato\nidentifier: urn:isbn:9780000000000\n---\n\nTexto.\n",
        )
        .expect("valid frontmatter");
        let metadata = document.metadata();
        assert_eq!(
            metadata.date().map(|date| date.to_string()),
            Some("2026-03-05".to_owned())
        );
        assert_eq!(metadata.subject().map(Subject::as_str), Some("Contrato"));
        assert_eq!(
            metadata
                .keywords()
                .iter()
                .map(Keyword::as_str)
                .collect::<Vec<_>>(),
            vec!["legal", "contrato"]
        );
        assert_eq!(
            metadata.identifier().map(Identifier::as_str),
            Some("urn:isbn:9780000000000")
        );
    }

    #[test]
    fn an_invalid_frontmatter_date_is_rejected() {
        let error = parse("---\ndate: 2025-02-29\n---\n").expect_err("an invalid date");
        assert!(matches!(
            error,
            MarkdownError::Metadata(MetadataError::InvalidDate)
        ));
    }

    #[test]
    fn a_standalone_pagebreak_marker_is_a_page_break() {
        for marker in ["-pagebreak-", "-!-"] {
            let source = format!("Antes.\n\n{marker}\n\nDepois.\n");
            let document = parse(&source).expect("a page break");
            assert_eq!(
                document.blocks(),
                &[
                    Block::Paragraph(vec![Inline::Text("Antes.".to_owned())]),
                    Block::PageBreak,
                    Block::Paragraph(vec![Inline::Text("Depois.".to_owned())]),
                ],
                "{marker}"
            );
        }
    }

    #[test]
    fn a_pagebreak_marker_inside_a_paragraph_stays_text() {
        for source in ["Texto -pagebreak- aqui.\n", "Texto -!- aqui.\n"] {
            let document = parse(source).expect("inline text");
            assert!(
                matches!(first_block(&document), Block::Paragraph(_)),
                "{source}"
            );
        }
    }

    #[test]
    fn the_old_brace_pagebreak_is_ordinary_text() {
        let document = parse("Antes.\n\n{pagebreak}\n\nDepois.\n").expect("ordinary text");
        assert!(
            matches!(document.blocks().get(1), Some(Block::Paragraph(_))),
            "the old marker must not become a page break"
        );
    }

    #[test]
    fn a_standalone_horizontal_marker_is_a_thematic_break() {
        let document = parse("Antes.\n\n-horizontal-\n\nDepois.\n").expect("a horizontal rule");
        assert_eq!(document.blocks().get(1), Some(&Block::ThematicBreak));
    }

    #[test]
    fn a_horizontal_marker_inside_a_paragraph_stays_text() {
        let document = parse("Texto -horizontal- aqui.\n").expect("inline text");
        assert!(matches!(first_block(&document), Block::Paragraph(_)));
    }

    #[test]
    fn the_standard_thematic_break_forms_are_blocks() {
        for marker in ["---", "***", "___"] {
            let source = format!("Antes.\n\n{marker}\n\nDepois.\n");
            let document = parse(&source).expect("a thematic break");
            assert!(
                document
                    .blocks()
                    .iter()
                    .any(|block| matches!(block, Block::ThematicBreak)),
                "{marker}"
            );
        }
    }

    #[test]
    fn a_setext_underline_is_a_heading_not_a_page_break() {
        let document = parse("Título\n===\n").expect("a setext heading");
        assert!(matches!(first_block(&document), Block::Heading(_)));
    }

    #[test]
    fn the_backslash_pagebreak_is_ordinary_text() {
        let document = parse("\\pagebreak\n").expect("ordinary text");
        assert!(matches!(first_block(&document), Block::Paragraph(_)));
    }

    #[test]
    fn unknown_frontmatter_fields_are_rejected() {
        let error =
            parse("---\ntitle: Contrato\nreviewer: Outro\n---\n").expect_err("an unknown field");
        assert!(matches!(error, MarkdownError::Frontmatter { .. }));
    }

    #[test]
    fn empty_metadata_values_are_rejected() {
        let error = parse("---\ntitle: \"\"\n---\n").expect_err("an empty title");
        assert!(matches!(
            error,
            MarkdownError::Metadata(MetadataError::EmptyTitle)
        ));
    }

    #[test]
    fn a_confidential_field_in_frontmatter_is_rejected() {
        for yaml in [
            "---\ntitle: \"{NOME}\"\n---\n",
            "---\nauthor: \"{NOME}\"\n---\n",
            "---\nsubject: \"{NOME}\"\n---\n",
            "---\nkeywords:\n  - \"{NOME}\"\n---\n",
            "---\nlanguage: \"{NOME}\"\n---\n",
            "---\ndate: \"{NOME}\"\n---\n",
        ] {
            let error = parse(yaml).expect_err("a confidential field in frontmatter");
            let message = error.to_string();
            assert!(message.contains("{NOME}"), "{message}");
            assert!(message.contains("frontmatter"), "{message}");
        }
    }

    #[test]
    fn a_lowercase_brace_in_frontmatter_is_ordinary_text() {
        let document = parse("---\ntitle: \"{nome}\"\n---\n").expect("a literal brace");
        assert_eq!(
            document.metadata().title().map(Title::as_str),
            Some("{nome}")
        );
    }

    #[test]
    fn headings_keep_their_level_and_content() {
        let document = parse("# Um\n\n### Três\n").expect("headings");
        let levels = document
            .blocks()
            .iter()
            .filter_map(|block| match block {
                Block::Heading(heading) => Some(heading.level().get()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(levels, vec![1, 3]);
    }

    #[test]
    fn inline_styles_are_preserved() {
        let document = parse("**forte** *ênfase* ~~riscado~~ `código`").expect("inlines");
        let inlines = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Strong(_)))
        );
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Emphasis(_)))
        );
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Strikethrough(_)))
        );
        assert!(
            inlines
                .iter()
                .any(|inline| matches!(inline, Inline::Code(_)))
        );
    }

    #[test]
    fn links_and_images_carry_destination_and_alternative() {
        let document = parse("[site](https://example.org \"Título\") ![logo](logo.png)")
            .expect("links and images");
        let inlines = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        let link = inlines
            .iter()
            .find_map(|inline| match inline {
                Inline::Link {
                    destination, title, ..
                } => Some((destination.as_str(), title.as_deref())),
                _ => None,
            })
            .expect("a link");
        assert_eq!(link, ("https://example.org", Some("Título")));
        let image = inlines
            .iter()
            .find_map(|inline| match inline {
                Inline::Image { alternative, .. } => Some(plain_text(alternative)),
                _ => None,
            })
            .expect("an image");
        assert_eq!(image, "logo");
    }

    #[test]
    fn reference_links_and_autolinks_become_links() {
        let document = parse(
            "[site][ref] <https://example.org> <mail@example.org>\n\n[ref]: https://example.org \"Exemplo\"\n",
        )
        .expect("reference links and autolinks");
        let inlines = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        let links: Vec<(&str, Option<&str>)> = inlines
            .iter()
            .filter_map(|inline| match inline {
                Inline::Link {
                    destination, title, ..
                } => Some((destination.as_str(), title.as_deref())),
                _ => None,
            })
            .collect();
        assert_eq!(
            links,
            vec![
                ("https://example.org", Some("Exemplo")),
                ("https://example.org", None),
                ("mail@example.org", None),
            ]
        );
    }

    #[test]
    fn entities_and_escapes_become_text() {
        let document = parse(r"\*literal\* &amp; &copy;").expect("entities and escapes");
        let inlines = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        assert_eq!(plain_text(inlines), "*literal* & ©");
    }

    #[test]
    fn extensions_outside_the_contract_stay_text() {
        let document = parse("==marca== ^sobrescrito^ $x^2$").expect("ordinary text");
        let inlines = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        assert_eq!(plain_text(inlines), "==marca== ^sobrescrito^ $x^2$");
    }

    #[test]
    fn single_and_double_tildes_are_strikethrough() {
        let document = parse("~riscado~ e ~~também~~").expect("strikethrough");
        let inlines = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        let spans = inlines
            .iter()
            .filter(|inline| matches!(inline, Inline::Strikethrough(_)))
            .count();
        assert_eq!(spans, 2, "both tilde forms are strikethrough: {inlines:?}");
    }

    #[test]
    fn alert_block_quotes_keep_their_kind() {
        let document = parse("> [!WARNING]\n> Cuidado.\n").expect("alert");
        let quote = match first_block(&document) {
            Block::Quote(quote) => quote,
            _ => panic!("expected a quote"),
        };
        assert_eq!(quote.kind(), QuoteKind::Alert(Alert::Warning));
        assert_eq!(quote.blocks().len(), 1);
    }

    #[test]
    fn plain_block_quotes_have_no_alert() {
        let document = parse("> Simples.\n").expect("quote");
        let quote = match first_block(&document) {
            Block::Quote(quote) => quote,
            _ => panic!("expected a quote"),
        };
        assert_eq!(quote.kind(), QuoteKind::Plain);
    }

    #[test]
    fn task_list_items_keep_their_state() {
        let document = parse("- [x] Feito\n- [ ] Pendente\n- Sem tarefa\n").expect("task list");
        let list = match first_block(&document) {
            Block::List(list) => list,
            _ => panic!("expected a list"),
        };
        let states = list.items().iter().map(ListItem::task).collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![Some(TaskState::Done), Some(TaskState::Todo), None]
        );
    }

    #[test]
    fn ordered_lists_keep_their_start() {
        let document = parse("3. Três\n4. Quatro\n").expect("ordered list");
        let list = match first_block(&document) {
            Block::List(list) => list,
            _ => panic!("expected a list"),
        };
        assert_eq!(list.start(), ListStart::Ordered(3));
        assert_eq!(list.items().len(), 2);
    }

    #[test]
    fn single_item_ordered_lists_become_numbered_paragraphs() {
        let document =
            parse("1. Requer a nulidade.\n\n2. Segue o pedido.\n").expect("numbered paragraphs");
        let numbered = document
            .blocks()
            .iter()
            .filter_map(|block| match block {
                Block::NumberedParagraph(paragraph) => {
                    Some((paragraph.number(), plain_text(paragraph.content())))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            numbered,
            vec![
                (1, "Requer a nulidade.".to_owned()),
                (2, "Segue o pedido.".to_owned()),
            ]
        );
    }

    #[test]
    fn multi_item_ordered_lists_stay_lists() {
        let document = parse("1. Um\n2. Dois\n").expect("a list");
        assert!(matches!(first_block(&document), Block::List(_)));
    }

    #[test]
    fn a_tight_single_item_list_is_a_numbered_paragraph() {
        let document = parse("1. Apenas um passo.\n").expect("one item");
        assert!(matches!(
            first_block(&document),
            Block::NumberedParagraph(_)
        ));
    }

    #[test]
    fn loose_numbered_paragraphs_increment_from_the_start() {
        let document = parse("7. Sete.\n\n8. Oito.\n").expect("numbered paragraphs");
        let numbers = document
            .blocks()
            .iter()
            .filter_map(|block| match block {
                Block::NumberedParagraph(paragraph) => Some(paragraph.number()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(numbers, vec![7, 8]);
    }

    #[test]
    fn a_loose_list_with_multi_paragraph_items_stays_a_list() {
        let document = parse("1. Um\n\n   Continuação.\n\n2. Dois\n").expect("a loose list");
        assert!(matches!(first_block(&document), Block::List(list) if !list.is_tight()));
    }

    #[test]
    fn tables_keep_alignments_header_and_rows() {
        let document = parse("| A | B |\n|:--|--:|\n| 1 | 2 |\n").expect("table");
        let table = match first_block(&document) {
            Block::Table(table) => table,
            _ => panic!("expected a table"),
        };
        assert_eq!(
            table.alignments(),
            &[ColumnAlignment::Left, ColumnAlignment::Right]
        );
        assert_eq!(plain_text(table.header()[0].content()), "A");
        assert_eq!(plain_text(table.rows()[0][1].content()), "2");
    }

    #[test]
    fn fenced_code_keeps_the_language_and_the_text() {
        let document = parse("```rust\nlet x = 1;\n```\n").expect("code block");
        let code = match first_block(&document) {
            Block::CodeBlock(code) => code,
            _ => panic!("expected a code block"),
        };
        assert_eq!(code.language().map(CodeLanguage::as_str), Some("rust"));
        assert_eq!(code.text(), "let x = 1;\n");
    }

    #[test]
    fn footnotes_are_linked_by_label() {
        let document = parse("Texto[^nota].\n\n[^nota]: Explicação.\n").expect("footnote");
        let paragraph = match first_block(&document) {
            Block::Paragraph(inlines) => inlines,
            _ => panic!("expected a paragraph"),
        };
        assert!(paragraph.iter().any(|inline| matches!(
            inline,
            Inline::FootnoteReference(id) if id.as_str() == "nota"
        )));
        let definition = document
            .blocks()
            .iter()
            .find_map(|block| match block {
                Block::FootnoteDefinition(definition) => Some(definition),
                _ => None,
            })
            .expect("a footnote definition");
        assert_eq!(definition.id().as_str(), "nota");
    }

    #[test]
    fn thematic_breaks_are_blocks() {
        let document = parse("Texto\n\n---\n\nMais\n").expect("thematic break");
        assert!(
            document
                .blocks()
                .iter()
                .any(|block| matches!(block, Block::ThematicBreak))
        );
    }

    #[test]
    fn raw_html_is_rejected() {
        let block = parse("<div>conteúdo</div>\n").expect_err("a raw HTML block");
        assert!(matches!(block, MarkdownError::HtmlNotSupported));
        let inline = parse("Um <b>termo</b> qualquer.\n").expect_err("inline raw HTML");
        assert!(matches!(inline, MarkdownError::HtmlNotSupported));
    }

    #[test]
    fn deep_nesting_is_rejected_instead_of_overflowing_the_stack() {
        let source = format!("{} texto\n", ">".repeat(MAX_NESTING + 1));
        let error = parse(&source).expect_err("deep nesting");
        assert!(matches!(
            error,
            MarkdownError::NestingTooDeep { limit: MAX_NESTING }
        ));
    }
}
