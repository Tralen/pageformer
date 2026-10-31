//! Backend-independent document model.
//!
//! The model holds semantic blocks and inlines and carries no layout decisions.
//! Backends read this model and never parse Markdown themselves.

use std::fmt;
use std::num::NonZeroU8;

use crate::image::{ImageAlignment, ImageSize};

/// A parsed Markdown document and its frontmatter metadata.
#[derive(Debug, Clone, PartialEq)]
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
    date: Option<DocumentDate>,
    subject: Option<Subject>,
    keywords: Vec<Keyword>,
    identifier: Option<Identifier>,
}

impl Metadata {
    pub fn new(title: Option<Title>, author: Option<Author>, language: Option<Language>) -> Self {
        Self {
            title,
            author,
            language,
            date: None,
            subject: None,
            keywords: Vec::new(),
            identifier: None,
        }
    }

    pub fn with_date(self, date: DocumentDate) -> Self {
        Self {
            date: Some(date),
            ..self
        }
    }

    pub fn with_subject(self, subject: Subject) -> Self {
        Self {
            subject: Some(subject),
            ..self
        }
    }

    pub fn with_keywords(self, keywords: Vec<Keyword>) -> Self {
        Self { keywords, ..self }
    }

    pub fn with_identifier(self, identifier: Identifier) -> Self {
        Self {
            identifier: Some(identifier),
            ..self
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

    pub fn date(&self) -> Option<DocumentDate> {
        self.date
    }

    pub fn subject(&self) -> Option<&Subject> {
        self.subject.as_ref()
    }

    pub fn keywords(&self) -> &[Keyword] {
        &self.keywords
    }

    pub fn identifier(&self) -> Option<&Identifier> {
        self.identifier.as_ref()
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

/// A non-empty document subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject(String);

impl Subject {
    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(MetadataError::EmptySubject)
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A non-empty document keyword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword(String);

impl Keyword {
    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(MetadataError::EmptyKeyword)
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A non-empty document identifier, such as an ISBN, a URN or a UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier(String);

impl Identifier {
    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(MetadataError::EmptyIdentifier);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(MetadataError::InvalidIdentifier);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A calendar date written in the frontmatter as `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentDate {
    year: i32,
    month: u8,
    day: u8,
}

impl DocumentDate {
    /// Builds a date from parts already validated by construction.
    /// This exists only for the fixed default date used by the PDF backend.
    pub(crate) const fn from_parts(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, MetadataError> {
        if !(1..=9999).contains(&year) {
            return Err(MetadataError::InvalidDate);
        }
        if !(1..=12).contains(&month) {
            return Err(MetadataError::InvalidDate);
        }
        if day < 1 || day > days_in_month(year, month) {
            return Err(MetadataError::InvalidDate);
        }
        Ok(Self { year, month, day })
    }

    pub fn parse(text: &str) -> Result<Self, MetadataError> {
        let trimmed = text.trim();
        let mut parts = trimmed.split('-');
        let year = parts.next().ok_or(MetadataError::InvalidDate)?;
        let month = parts.next().ok_or(MetadataError::InvalidDate)?;
        let day = parts.next().ok_or(MetadataError::InvalidDate)?;
        if parts.next().is_some()
            || year.len() != 4
            || month.len() != 2
            || day.len() != 2
            || !year.chars().all(|character| character.is_ascii_digit())
            || !month.chars().all(|character| character.is_ascii_digit())
            || !day.chars().all(|character| character.is_ascii_digit())
        {
            return Err(MetadataError::InvalidDate);
        }
        let year = year.parse().map_err(|_| MetadataError::InvalidDate)?;
        let month = month.parse().map_err(|_| MetadataError::InvalidDate)?;
        let day = day.parse().map_err(|_| MetadataError::InvalidDate)?;
        Self::new(year, month, day)
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }
}

impl fmt::Display for DocumentDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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
    EmptySubject,
    EmptyKeyword,
    EmptyIdentifier,
    InvalidLanguageTag,
    InvalidIdentifier,
    InvalidDate,
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyTitle => "the document title is empty",
            Self::EmptyAuthor => "the document author is empty",
            Self::EmptyLanguage => "the document language is empty",
            Self::EmptySubject => "the document subject is empty",
            Self::EmptyKeyword => "a document keyword is empty",
            Self::EmptyIdentifier => "the document identifier is empty",
            Self::InvalidLanguageTag => "the document language is not a tag such as `pt-BR`",
            Self::InvalidIdentifier => "the document identifier contains a control character",
            Self::InvalidDate => "the document date is not a calendar date such as `2026-03-05`",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for MetadataError {}

/// A semantic block of the document.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Block {
    Heading(Heading),
    Paragraph(Vec<Inline>),
    Image(ImageBlock),
    NumberedParagraph(NumberedParagraph),
    List(List),
    Table(Table),
    Quote(Quote),
    CodeBlock(CodeBlock),
    ThematicBreak,
    PageBreak,
    FootnoteDefinition(FootnoteDefinition),
}

/// A standalone image with its own block geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageBlock {
    destination: Destination,
    title: Option<String>,
    alternative: Vec<Inline>,
    size: ImageSize,
    alignment: Option<ImageAlignment>,
}

impl ImageBlock {
    pub fn new(
        destination: Destination,
        title: Option<String>,
        alternative: Vec<Inline>,
        size: ImageSize,
        alignment: Option<ImageAlignment>,
    ) -> Self {
        Self {
            destination,
            title,
            alternative,
            size,
            alignment,
        }
    }

    pub fn destination(&self) -> &Destination {
        &self.destination
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn alternative(&self) -> &[Inline] {
        &self.alternative
    }

    pub fn size(&self) -> ImageSize {
        self.size
    }

    pub fn alignment(&self) -> Option<ImageAlignment> {
        self.alignment
    }
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
#[derive(Debug, Clone, PartialEq)]
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

    /// The spacing used for one list item by the paged renderers.
    pub fn item_spacing(&self) -> ItemSpacing {
        if self.tight {
            ItemSpacing::Tight
        } else {
            ItemSpacing::Loose
        }
    }

    /// Whether every item carries a task state.
    pub fn is_task_list(&self) -> bool {
        !self.items.is_empty() && self.items.iter().all(|item| item.task().is_some())
    }
}

/// How a renderer spaces one list item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemSpacing {
    Tight,
    Loose,
}

/// How a list starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStart {
    Unordered,
    Ordered(u64),
}

/// One list item and its optional task state.
#[derive(Debug, Clone, PartialEq)]
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

/// Whether a table cell belongs to the header row or the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Header,
    Data,
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
#[derive(Debug, Clone, PartialEq)]
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

impl Alert {
    /// The label shown in brackets at the top of an alert quote.
    pub fn label(self) -> &'static str {
        match self {
            Self::Note => "!NOTE",
            Self::Tip => "!TIP",
            Self::Important => "!IMPORTANT",
            Self::Warning => "!WARNING",
            Self::Caution => "!CAUTION",
        }
    }
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
#[derive(Debug, Clone, PartialEq)]
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

    /// Classifies the destination without resolving it.
    ///
    /// A destination without a scheme, or with a colon only after a path or
    /// fragment separator, is relative. A destination with a scheme is compared
    /// case-insensitively, and a destination with a control character is never
    /// allowed.
    pub fn scheme(&self) -> LinkScheme {
        let text = self.0.trim();
        if text.chars().any(char::is_control) {
            return LinkScheme::Other;
        }
        let Some(colon) = text.find(':') else {
            return LinkScheme::Relative;
        };
        let authority = &text[..colon];
        if authority.contains(['/', '?', '#']) {
            return LinkScheme::Relative;
        }
        let mut characters = authority.chars();
        let Some(first) = characters.next() else {
            return LinkScheme::Other;
        };
        if !first.is_ascii_alphabetic()
            || !characters.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
        {
            return LinkScheme::Other;
        }
        match authority.to_ascii_lowercase().as_str() {
            "http" => LinkScheme::Http,
            "https" => LinkScheme::Https,
            "mailto" => LinkScheme::Mailto,
            _ => LinkScheme::Other,
        }
    }
}

/// The scheme of a destination, as far as a reflowable backend needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkScheme {
    Relative,
    Http,
    Https,
    Mailto,
    Other,
}

impl LinkScheme {
    /// Whether an external link may be emitted without a security check.
    pub fn is_allowed(self) -> bool {
        matches!(
            self,
            Self::Relative | Self::Http | Self::Https | Self::Mailto
        )
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
    fn a_task_list_needs_every_item_to_carry_a_state() {
        let item = |task| ListItem::new(task, Vec::new());
        assert!(
            List::new(
                ListStart::Unordered,
                vec![item(Some(TaskState::Done)), item(Some(TaskState::Todo))],
                true
            )
            .is_task_list()
        );
        assert!(
            !List::new(
                ListStart::Unordered,
                vec![item(Some(TaskState::Done)), item(None)],
                true
            )
            .is_task_list()
        );
        assert!(!List::new(ListStart::Unordered, Vec::new(), true).is_task_list());
    }

    #[test]
    fn alert_labels_are_uppercase() {
        assert_eq!(Alert::Warning.label(), "!WARNING");
        assert_eq!(Alert::Caution.label(), "!CAUTION");
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
    fn a_date_accepts_a_calendar_day() {
        let date = DocumentDate::parse("2026-03-05").expect("a valid date");
        assert_eq!((date.year(), date.month(), date.day()), (2026, 3, 5));
        assert_eq!(date.to_string(), "2026-03-05");
        assert!(DocumentDate::parse("2024-02-29").is_ok());
    }

    #[test]
    fn a_date_rejects_a_day_outside_the_calendar() {
        for text in [
            "2025-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026-3-05",
            "26-03-05",
            "2026-03-05T10:00",
            "2026/03/05",
            "",
            "0000-01-01",
        ] {
            assert_eq!(
                DocumentDate::parse(text),
                Err(MetadataError::InvalidDate),
                "`{text}` should not be a valid date"
            );
        }
    }

    #[test]
    fn subject_and_keywords_cannot_be_empty() {
        assert_eq!(Subject::parse("  "), Err(MetadataError::EmptySubject));
        assert_eq!(
            Subject::parse(" Contrato ").map(|subject| subject.as_str().to_owned()),
            Ok("Contrato".to_owned())
        );
        assert_eq!(Keyword::parse(" "), Err(MetadataError::EmptyKeyword));
        assert_eq!(
            Keyword::parse(" legal ").map(|keyword| keyword.as_str().to_owned()),
            Ok("legal".to_owned())
        );
    }

    #[test]
    fn an_identifier_is_trimmed_and_rejects_empty_or_control_text() {
        assert_eq!(
            Identifier::parse("  urn:isbn:9780000000000  ")
                .map(|identifier| { identifier.as_str().to_owned() }),
            Ok("urn:isbn:9780000000000".to_owned())
        );
        assert_eq!(Identifier::parse("  "), Err(MetadataError::EmptyIdentifier));
        assert_eq!(
            Identifier::parse("a\u{1}b"),
            Err(MetadataError::InvalidIdentifier)
        );
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

    #[test]
    fn a_destination_scheme_is_classified_conservatively() {
        let scheme = |text| Destination::new(text).scheme();

        assert_eq!(scheme("https://example.org"), LinkScheme::Https);
        assert_eq!(scheme("HTTP://example.org"), LinkScheme::Http);
        assert_eq!(scheme("mailto:someone@example.org"), LinkScheme::Mailto);
        assert_eq!(scheme("docs/contrato.md"), LinkScheme::Relative);
        assert_eq!(scheme("/docs/contrato.md"), LinkScheme::Relative);
        assert_eq!(scheme("#nota"), LinkScheme::Relative);
        assert_eq!(scheme("docs/a:b.md"), LinkScheme::Relative);
        assert_eq!(scheme("javascript:alert(1)"), LinkScheme::Other);
        assert_eq!(scheme("JaVaScRiPt:alert(1)"), LinkScheme::Other);
        assert_eq!(scheme("data:text/html,hello"), LinkScheme::Other);
        assert_eq!(scheme("file:///etc/passwd"), LinkScheme::Other);
        assert_eq!(scheme("java\nscript:alert(1)"), LinkScheme::Other);
        assert_eq!(scheme("://missing"), LinkScheme::Other);
    }

    #[test]
    fn only_web_schemes_and_relative_destinations_are_allowed() {
        assert!(LinkScheme::Relative.is_allowed());
        assert!(LinkScheme::Http.is_allowed());
        assert!(LinkScheme::Https.is_allowed());
        assert!(LinkScheme::Mailto.is_allowed());
        assert!(!LinkScheme::Other.is_allowed());
    }
}
