//! Backend-independent document model.
//!
//! The model holds semantic blocks and inlines and carries no layout decisions.
//! Backends read this model and never parse Markdown themselves.

use std::fmt;
use std::num::NonZeroU8;

/// A parsed Markdown document and its frontmatter metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    metadata: Metadata,
    blocks: Vec<Block>,
}

impl Document {
    pub fn new(metadata: Metadata, blocks: Vec<Block>) -> Self {
        Self { metadata, blocks }
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn heading_count(&self) -> usize {
        fn count(blocks: &[Block]) -> usize {
            blocks
                .iter()
                .map(|block| match block {
                    Block::Heading(_) => 1,
                    Block::List(list) => list.items().iter().map(|item| count(item.blocks())).sum(),
                    Block::Quote(quote) => count(quote.blocks()),
                    Block::FootnoteDefinition(definition) => count(definition.blocks()),
                    _ => 0,
                })
                .sum()
        }
        count(&self.blocks)
    }
}

/// Metadata that comes from the Markdown frontmatter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    title: Option<Title>,
    author: Option<Author>,
    language: Option<Language>,
}

impl Metadata {
    pub fn new(title: Option<Title>, author: Option<Author>, language: Option<Language>) -> Self {
        Self {
            title,
            author,
            language,
        }
    }

    pub fn title(&self) -> Option<&Title> {
        self.title.as_ref()
    }

    pub fn author(&self) -> Option<&Author> {
        self.author.as_ref()
    }

    pub fn language(&self) -> Option<&Language> {
        self.language.as_ref()
    }
}

/// A non-empty document title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title(String);

impl Title {
    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(MetadataError::EmptyTitle)
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Title {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A non-empty document author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author(String);

impl Author {
    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(MetadataError::EmptyAuthor)
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Author {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A BCP 47 style language tag, such as `pt-BR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language(String);

impl Language {
    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(MetadataError::EmptyLanguage);
        }
        if is_language_tag(trimmed) {
            Ok(Self(trimmed.to_owned()))
        } else {
            Err(MetadataError::InvalidLanguageTag)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Only simple subtags are accepted, which covers the languages this project
/// documents. Full BCP 47 validation is out of scope.
fn is_language_tag(tag: &str) -> bool {
    let mut subtags = tag.split('-');
    let primary_is_valid = subtags.next().is_some_and(|primary| {
        (2..=3).contains(&primary.len())
            && primary
                .chars()
                .all(|character| character.is_ascii_alphabetic())
    });
    primary_is_valid
        && subtags.all(|subtag| {
            (1..=8).contains(&subtag.len())
                && subtag
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
}

/// Why a metadata value was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataError {
    EmptyTitle,
    EmptyAuthor,
    EmptyLanguage,
    InvalidLanguageTag,
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyTitle => "the document title is empty",
            Self::EmptyAuthor => "the document author is empty",
            Self::EmptyLanguage => "the document language is empty",
            Self::InvalidLanguageTag => "the document language is not a tag such as `pt-BR`",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for MetadataError {}

/// A semantic block of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Block {
    Heading(Heading),
    Paragraph(Vec<Inline>),
    NumberedParagraph(NumberedParagraph),
    List(List),
    Table(Table),
    Quote(Quote),
    CodeBlock(CodeBlock),
    ThematicBreak,
    FootnoteDefinition(FootnoteDefinition),
}

/// A numbered paragraph, such as the manual numbering in legal documents.
///
/// A single-item ordered list becomes this block, so the source can write
/// `1.` without escaping the period and without being treated as a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedParagraph {
    number: u64,
    content: Vec<Inline>,
}

impl NumberedParagraph {
    pub fn new(number: u64, content: Vec<Inline>) -> Self {
        Self { number, content }
    }

    pub fn number(&self) -> u64 {
        self.number
    }

    pub fn content(&self) -> &[Inline] {
        &self.content
    }
}

/// A heading from level 1 to 6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    level: HeadingLevel,
    content: Vec<Inline>,
}

impl Heading {
    pub fn new(level: HeadingLevel, content: Vec<Inline>) -> Self {
        Self { level, content }
    }

    pub fn level(&self) -> HeadingLevel {
        self.level
    }

    pub fn content(&self) -> &[Inline] {
        &self.content
    }
}

/// A heading level from 1 to 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadingLevel(NonZeroU8);

impl HeadingLevel {
    pub const MAX: u8 = 6;

    pub fn new(level: u8) -> Option<Self> {
        match level {
            1..=Self::MAX => NonZeroU8::new(level).map(Self),
            _ => None,
        }
    }

    pub fn get(self) -> u8 {
        self.0.get()
    }
}

/// A list, ordered or unordered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    start: ListStart,
    items: Vec<ListItem>,
    tight: bool,
}

impl List {
    pub fn new(start: ListStart, items: Vec<ListItem>, tight: bool) -> Self {
        Self {
            start,
            items,
            tight,
        }
    }

    pub fn start(&self) -> ListStart {
        self.start
    }

    pub fn items(&self) -> &[ListItem] {
        &self.items
    }

    /// Whether the items are spaced as a compact list.
    pub fn is_tight(&self) -> bool {
        self.tight
    }
}

/// How a list starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStart {
    Unordered,
    Ordered(u64),
}

/// One list item and its optional task state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    task: Option<TaskState>,
    blocks: Vec<Block>,
}

impl ListItem {
    pub fn new(task: Option<TaskState>, blocks: Vec<Block>) -> Self {
        Self { task, blocks }
    }

    pub fn task(&self) -> Option<TaskState> {
        self.task
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
}

/// The state of a task list item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Todo,
    Done,
}

/// A table with a header row and zero or more body rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    alignments: Vec<ColumnAlignment>,
    header: Vec<TableCell>,
    rows: Vec<Vec<TableCell>>,
}

impl Table {
    /// Every row must have exactly one cell per declared column.
    pub fn new(
        alignments: Vec<ColumnAlignment>,
        header: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
    ) -> Result<Self, TableError> {
        if alignments.is_empty() {
            return Err(TableError::NoColumns);
        }
        let width = alignments.len();
        if header.len() != width {
            return Err(TableError::HeaderMismatch {
                expected: width,
                found: header.len(),
            });
        }
        for (index, row) in rows.iter().enumerate() {
            if row.len() != width {
                return Err(TableError::RowMismatch {
                    row: index,
                    expected: width,
                    found: row.len(),
                });
            }
        }
        Ok(Self {
            alignments,
            header,
            rows,
        })
    }

    pub fn alignments(&self) -> &[ColumnAlignment] {
        &self.alignments
    }

    pub fn header(&self) -> &[TableCell] {
        &self.header
    }

    pub fn rows(&self) -> &[Vec<TableCell>] {
        &self.rows
    }
}

/// One table cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    content: Vec<Inline>,
}

impl TableCell {
    pub fn new(content: Vec<Inline>) -> Self {
        Self { content }
    }

    pub fn content(&self) -> &[Inline] {
        &self.content
    }
}

/// Why a table was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableError {
    NoColumns,
    HeaderMismatch {
        expected: usize,
        found: usize,
    },
    RowMismatch {
        row: usize,
        expected: usize,
        found: usize,
    },
}

impl fmt::Display for TableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoColumns => formatter.write_str("the table has no columns"),
            Self::HeaderMismatch { expected, found } => {
                write!(
                    formatter,
                    "the header has {found} cells instead of {expected}"
                )
            }
            Self::RowMismatch {
                row,
                expected,
                found,
            } => write!(
                formatter,
                "row {row} has {found} cells instead of {expected}"
            ),
        }
    }
}

impl std::error::Error for TableError {}

/// The text alignment declared for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlignment {
    Default,
    Left,
    Center,
    Right,
}

/// A block quote, optionally tagged as a GFM alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    kind: QuoteKind,
    blocks: Vec<Block>,
}

impl Quote {
    pub fn new(kind: QuoteKind, blocks: Vec<Block>) -> Self {
        Self { kind, blocks }
    }

    pub fn kind(&self) -> QuoteKind {
        self.kind
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
}

/// The kind of a block quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteKind {
    Plain,
    Alert(Alert),
}

/// The GFM alert variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alert {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

/// A fenced or indented code block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    language: Option<CodeLanguage>,
    text: String,
}

impl CodeBlock {
    pub fn new(language: Option<CodeLanguage>, text: String) -> Self {
        Self { language, text }
    }

    pub fn language(&self) -> Option<&CodeLanguage> {
        self.language.as_ref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// The first token of a fenced code block info string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLanguage(String);

impl CodeLanguage {
    pub fn parse(info: &str) -> Option<Self> {
        info.split_whitespace()
            .next()
            .map(|token| Self(token.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A footnote definition and the blocks that belong to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteDefinition {
    id: FootnoteId,
    blocks: Vec<Block>,
}

impl FootnoteDefinition {
    pub fn new(id: FootnoteId, blocks: Vec<Block>) -> Self {
        Self { id, blocks }
    }

    pub fn id(&self) -> &FootnoteId {
        &self.id
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
}

/// The label of a footnote definition or reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteId(String);

impl FootnoteId {
    pub fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic inline content.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Inline {
    Text(String),
    Code(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link {
        destination: Destination,
        title: Option<String>,
        content: Vec<Inline>,
    },
    Image {
        destination: Destination,
        title: Option<String>,
        alternative: Vec<Inline>,
    },
    SoftBreak,
    HardBreak,
    FootnoteReference(FootnoteId),
}

/// The destination of a link or image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination(String);

impl Destination {
    pub fn new(url: &str) -> Self {
        Self(url.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed_and_cannot_be_empty() {
        let title = Title::parse("  Contrato  ").expect("valid title");
        assert_eq!(title.as_str(), "Contrato");
        assert_eq!(Title::parse("   "), Err(MetadataError::EmptyTitle));
    }

    #[test]
    fn author_cannot_be_empty() {
        assert_eq!(Author::parse(""), Err(MetadataError::EmptyAuthor));
        assert_eq!(
            Author::parse("Pageformer").expect("valid author").as_str(),
            "Pageformer"
        );
    }

    #[test]
    fn language_accepts_simple_tags() {
        for tag in ["pt-BR", "en", "zh-Hant-TW", "es-419"] {
            let language = Language::parse(tag).expect("valid language");
            assert_eq!(language.as_str(), tag);
        }
    }

    #[test]
    fn language_rejects_malformed_tags() {
        for tag in ["", "p", "pt_BR", "pt-", "pt--BR", "1pt", "pt BR"] {
            assert!(
                Language::parse(tag).is_err(),
                "`{tag}` should not be a valid language"
            );
        }
    }

    #[test]
    fn heading_level_is_limited_to_six() {
        assert_eq!(HeadingLevel::new(0), None);
        assert_eq!(HeadingLevel::new(1).map(HeadingLevel::get), Some(1));
        assert_eq!(HeadingLevel::new(6).map(HeadingLevel::get), Some(6));
        assert_eq!(HeadingLevel::new(7), None);
    }

    #[test]
    fn table_rejects_rows_with_the_wrong_width() {
        let cell = || TableCell::new(Vec::new());
        let error = Table::new(
            vec![ColumnAlignment::Default; 2],
            vec![cell(), cell()],
            vec![vec![cell()]],
        );
        assert_eq!(
            error,
            Err(TableError::RowMismatch {
                row: 0,
                expected: 2,
                found: 1,
            })
        );
    }

    #[test]
    fn table_rejects_a_table_without_columns() {
        let error = Table::new(Vec::new(), Vec::new(), Vec::new());
        assert_eq!(error, Err(TableError::NoColumns));
    }

    #[test]
    fn table_rejects_a_header_with_the_wrong_width() {
        let error = Table::new(vec![ColumnAlignment::Default], Vec::new(), Vec::new());
        assert_eq!(
            error,
            Err(TableError::HeaderMismatch {
                expected: 1,
                found: 0,
            })
        );
    }

    #[test]
    fn code_language_uses_the_first_info_token() {
        assert_eq!(
            CodeLanguage::parse("rust,linenos").map(|language| language.as_str().to_owned()),
            Some("rust,linenos".to_owned())
        );
        assert_eq!(CodeLanguage::parse("   "), None);
    }

    #[test]
    fn document_counts_headings() {
        let heading = |level| {
            Block::Heading(Heading::new(
                HeadingLevel::new(level).expect("valid level"),
                Vec::new(),
            ))
        };
        let document = Document::new(
            Metadata::default(),
            vec![
                heading(1),
                Block::ThematicBreak,
                heading(2),
                Block::ThematicBreak,
            ],
        );
        assert_eq!(document.block_count(), 4);
        assert_eq!(document.heading_count(), 2);
    }
}
