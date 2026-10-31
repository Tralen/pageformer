//! Confidential fields: `{NAME}` placeholders resolved before any backend.
//!
//! The store is built by the loader, never from arbitrary strings. A value
//! never appears in `Debug`, `Display`, errors or diagnostics, and the resolved
//! document is the only place where the text is expanded.

pub mod config;
pub mod loader;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::document::{
    Block, Document, FootnoteDefinition, Heading, Inline, List, ListItem, NumberedParagraph, Quote,
    Table, TableCell,
};

/// A confidential field name, written as an uppercase identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfidentialName(String);

impl ConfidentialName {
    /// Parses a field name, such as `NOME_COMPLETO`, from a placeholder body.
    pub fn parse(text: &str) -> Option<Self> {
        let mut characters = text.chars();
        let first = characters.next()?;
        if !first.is_ascii_uppercase() {
            return None;
        }
        if characters.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        }) {
            Some(Self(text.to_owned()))
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConfidentialName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A confidential value that never prints itself.
#[derive(Clone, PartialEq, Eq)]
pub struct ConfidentialValue(String);

impl ConfidentialValue {
    /// Builds a value. Only the loader calls this.
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The text inserted into the document. Only the resolver calls this.
    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ConfidentialValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

impl fmt::Display for ConfidentialValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

/// The confidential values available to one run.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ConfidentialStore {
    values: BTreeMap<ConfidentialName, ConfidentialValue>,
}

impl ConfidentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(&mut self, name: ConfidentialName, value: ConfidentialValue) {
        self.values.insert(name, value);
    }

    fn get(&self, name: &ConfidentialName) -> Option<&ConfidentialValue> {
        self.values.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &ConfidentialName> {
        self.values.keys()
    }

    /// Replaces every known value in a message with a fixed marker.
    ///
    /// The scan is sequential, so a replacement never feeds the next one.
    pub fn redact(&self, text: &str) -> String {
        let mut values = self
            .values
            .values()
            .map(ConfidentialValue::expose)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        let mut redacted = String::with_capacity(text.len());
        let mut rest = text;
        loop {
            let next = values
                .iter()
                .filter_map(|value| rest.find(*value).map(|index| (index, *value)))
                .min_by_key(|(index, _)| *index);
            match next {
                Some((index, value)) => {
                    redacted.push_str(rest.get(..index).unwrap_or_default());
                    redacted.push_str("[redacted]");
                    rest = rest.get(index + value.len()..).unwrap_or_default();
                }
                None => {
                    redacted.push_str(rest);
                    return redacted;
                }
            }
        }
    }
}

impl fmt::Debug for ConfidentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfidentialStore")
            .field("fields", &self.values.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Whether `child` resolves inside `parent`.
///
/// A path that cannot be normalized is treated as inside, so a caller that
/// uses this as a safety check fails closed.
pub fn is_within(parent: &Path, child: &Path) -> bool {
    match (normalize_absolute(parent), normalize_absolute(child)) {
        (Some(parent), Some(child)) => child.starts_with(parent),
        _ => true,
    }
}

/// Resolves `.` and `..`, follows symlinks on the existing prefix and keeps a
/// missing suffix.
pub(crate) fn normalize_absolute(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut current = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        if let Ok(canonical) = current.canonicalize() {
            let mut normalized = canonical;
            for component in suffix.iter().rev() {
                normalized.push(component);
            }
            return Some(normalized);
        }
        let name = current.file_name()?;
        suffix.push(name.to_owned());
        current = current.parent()?;
    }
}

/// A document with its confidential fields resolved and the names that were
/// found, in a deterministic order.
#[derive(Clone, PartialEq)]
pub struct Resolution {
    document: Document,
    names: Vec<ConfidentialName>,
}

impl Resolution {
    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn into_document(self) -> Document {
        self.document
    }

    pub fn names(&self) -> &[ConfidentialName] {
        &self.names
    }
}

impl fmt::Debug for Resolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Resolution")
            .field("names", &self.names)
            .field("blocks", &self.document.block_count())
            .finish()
    }
}

/// Why a document could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    UnknownField { name: String, location: String },
    InvalidTable { location: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownField { name, location } => {
                write!(
                    formatter,
                    "unknown confidential field `{{{name}}}` at {location}"
                )
            }
            Self::InvalidTable { location } => {
                write!(formatter, "the table at {location} could not be rebuilt")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolves every text placeholder in a document.
///
/// Only text inlines are scanned. Code spans, code blocks, link destinations,
/// link titles, image destinations and frontmatter are left untouched, and a
/// substituted value is never scanned again.
pub fn resolve(document: &Document, store: &ConfidentialStore) -> Result<Resolution, ResolveError> {
    let mut names = BTreeMap::new();
    let blocks = resolve_blocks(document.blocks(), store, &mut names, "block")?;
    Ok(Resolution {
        document: Document::new(document.metadata().clone(), blocks),
        names: names.into_keys().collect(),
    })
}

/// The first confidential field found in a text, if any.
pub(crate) fn first_field(text: &str) -> Option<String> {
    fields(text)
        .into_iter()
        .next()
        .map(|(_, _, name)| name.as_str().to_owned())
}

fn resolve_blocks(
    blocks: &[Block],
    store: &ConfidentialStore,
    names: &mut BTreeMap<ConfidentialName, ()>,
    prefix: &str,
) -> Result<Vec<Block>, ResolveError> {
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let location = format!("{prefix} {}", index + 1);
            resolve_block(block, store, names, &location)
        })
        .collect()
}

fn resolve_block(
    block: &Block,
    store: &ConfidentialStore,
    names: &mut BTreeMap<ConfidentialName, ()>,
    location: &str,
) -> Result<Block, ResolveError> {
    match block {
        Block::Heading(heading) => {
            let content = resolve_inlines(heading.content(), store, names, location)?;
            Ok(Block::Heading(Heading::new(heading.level(), content)))
        }
        Block::Paragraph(inlines) => Ok(Block::Paragraph(resolve_inlines(
            inlines, store, names, location,
        )?)),
        Block::Image(image) => Ok(Block::Image(crate::document::ImageBlock::new(
            image.destination().clone(),
            image.title().map(str::to_owned),
            resolve_inlines(image.alternative(), store, names, location)?,
            image.size(),
            image.alignment(),
        ))),
        Block::NumberedParagraph(paragraph) => {
            let content = resolve_inlines(paragraph.content(), store, names, location)?;
            Ok(Block::NumberedParagraph(NumberedParagraph::new(
                paragraph.number(),
                content,
            )))
        }
        Block::List(list) => {
            let items = list
                .items()
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let item_location = format!("{location}, list item {}", index + 1);
                    let blocks = resolve_blocks(item.blocks(), store, names, &item_location)?;
                    Ok(ListItem::new(item.task(), blocks))
                })
                .collect::<Result<Vec<_>, ResolveError>>()?;
            Ok(Block::List(List::new(list.start(), items, list.is_tight())))
        }
        Block::Table(table) => {
            let table_location = format!("{location}, table");
            let header = resolve_cells(table.header(), store, names, &table_location)?;
            let rows = table
                .rows()
                .iter()
                .map(|row| resolve_cells(row, store, names, &table_location))
                .collect::<Result<Vec<_>, _>>()?;
            let table = Table::new(table.alignments().to_vec(), header, rows).map_err(|_| {
                ResolveError::InvalidTable {
                    location: table_location,
                }
            })?;
            Ok(Block::Table(table))
        }
        Block::Quote(quote) => {
            let quote_location = format!("{location}, quote");
            let blocks = resolve_blocks(quote.blocks(), store, names, &quote_location)?;
            Ok(Block::Quote(Quote::new(quote.kind(), blocks)))
        }
        Block::FootnoteDefinition(definition) => {
            let footnote_location = format!("{location}, footnote");
            let blocks = resolve_blocks(definition.blocks(), store, names, &footnote_location)?;
            Ok(Block::FootnoteDefinition(FootnoteDefinition::new(
                definition.id().clone(),
                blocks,
            )))
        }
        Block::CodeBlock(_) | Block::ThematicBreak | Block::PageBreak => Ok(block.clone()),
    }
}

fn resolve_cells(
    cells: &[TableCell],
    store: &ConfidentialStore,
    names: &mut BTreeMap<ConfidentialName, ()>,
    location: &str,
) -> Result<Vec<TableCell>, ResolveError> {
    cells
        .iter()
        .map(|cell| {
            let content = resolve_inlines(cell.content(), store, names, location)?;
            Ok(TableCell::new(content))
        })
        .collect()
}

fn resolve_inlines(
    inlines: &[Inline],
    store: &ConfidentialStore,
    names: &mut BTreeMap<ConfidentialName, ()>,
    location: &str,
) -> Result<Vec<Inline>, ResolveError> {
    inlines
        .iter()
        .map(|inline| resolve_inline(inline, store, names, location))
        .collect()
}

fn resolve_inline(
    inline: &Inline,
    store: &ConfidentialStore,
    names: &mut BTreeMap<ConfidentialName, ()>,
    location: &str,
) -> Result<Inline, ResolveError> {
    match inline {
        Inline::Text(text) => Ok(Inline::Text(substitute(text, store, names, location)?)),
        Inline::Code(_) => Ok(inline.clone()),
        Inline::Emphasis(content) => Ok(Inline::Emphasis(resolve_inlines(
            content, store, names, location,
        )?)),
        Inline::Strong(content) => Ok(Inline::Strong(resolve_inlines(
            content, store, names, location,
        )?)),
        Inline::Strikethrough(content) => Ok(Inline::Strikethrough(resolve_inlines(
            content, store, names, location,
        )?)),
        Inline::Link {
            destination,
            title,
            content,
        } => Ok(Inline::Link {
            destination: destination.clone(),
            title: title.clone(),
            content: resolve_inlines(content, store, names, location)?,
        }),
        Inline::Image {
            destination,
            title,
            alternative,
        } => Ok(Inline::Image {
            destination: destination.clone(),
            title: title.clone(),
            alternative: resolve_inlines(alternative, store, names, location)?,
        }),
        Inline::SoftBreak | Inline::HardBreak | Inline::FootnoteReference(_) => Ok(inline.clone()),
    }
}

fn substitute(
    text: &str,
    store: &ConfidentialStore,
    names: &mut BTreeMap<ConfidentialName, ()>,
    location: &str,
) -> Result<String, ResolveError> {
    let fields = fields(text);
    if fields.is_empty() {
        return Ok(text.to_owned());
    }
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end, name) in fields {
        output.push_str(text.get(cursor..start).unwrap_or_default());
        let value = store.get(&name).ok_or_else(|| ResolveError::UnknownField {
            name: name.as_str().to_owned(),
            location: location.to_owned(),
        })?;
        output.push_str(value.expose());
        names.insert(name, ());
        cursor = end;
    }
    output.push_str(text.get(cursor..).unwrap_or_default());
    Ok(output)
}

fn fields(text: &str) -> Vec<(usize, usize, ConfidentialName)> {
    let mut fields = Vec::new();
    let mut index = 0;
    while let Some(relative) = text.get(index..).and_then(|rest| rest.find('{')) {
        let open = index + relative;
        let after = text.get(open + 1..).unwrap_or_default();
        if let Some(close) = after.find('}') {
            let token = after.get(..close).unwrap_or_default();
            if let Some(name) = ConfidentialName::parse(token) {
                fields.push((open, open + 1 + close + 1, name));
            }
        }
        index = open + 1;
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ColumnAlignment, HeadingLevel, ListStart, Metadata, TaskState, Title};

    fn store() -> ConfidentialStore {
        let mut store = ConfidentialStore::new();
        store.insert(
            ConfidentialName::parse("NOME").expect("a name"),
            ConfidentialValue::new("Fulano de Tal"),
        );
        store.insert(
            ConfidentialName::parse("CPF").expect("a name"),
            ConfidentialValue::new("000.000.000-00"),
        );
        store
    }

    fn resolve_text(text: &str) -> Result<Resolution, ResolveError> {
        let document = Document::new(
            Metadata::default(),
            vec![Block::Paragraph(vec![Inline::Text(text.to_owned())])],
        );
        resolve(&document, &store())
    }

    #[test]
    fn names_follow_the_uppercase_pattern() {
        assert!(ConfidentialName::parse("NOME").is_some());
        assert!(ConfidentialName::parse("NOME_COMPLETO_2").is_some());
        for text in ["nome", "Nome", "1NOME", "NOME-COMPLETO", "NOME ", ""] {
            assert!(
                ConfidentialName::parse(text).is_none(),
                "`{text}` should not be a name"
            );
        }
    }

    #[test]
    fn a_value_never_prints_itself() {
        let value = ConfidentialValue::new("segredo");
        assert_eq!(format!("{value:?}"), "[redacted]");
        assert_eq!(format!("{value}"), "[redacted]");
    }

    #[test]
    fn text_placeholders_are_substituted() {
        let resolution = resolve_text("Nome: {NOME}; CPF: {CPF}.").expect("resolution");
        let text = match resolution.document().blocks() {
            [Block::Paragraph(inlines)] => match inlines.as_slice() {
                [Inline::Text(text)] => text,
                other => panic!("expected text, found {other:?}"),
            },
            other => panic!("expected a paragraph, found {other:?}"),
        };
        assert_eq!(text, "Nome: Fulano de Tal; CPF: 000.000.000-00.");
        assert_eq!(
            resolution
                .names()
                .iter()
                .map(ConfidentialName::as_str)
                .collect::<Vec<_>>(),
            vec!["CPF", "NOME"]
        );
    }

    #[test]
    fn unknown_fields_are_reported_by_name_and_location() {
        let error = resolve_text("Nome: {DESCONHECIDO}.").expect_err("an unknown field");
        assert_eq!(
            error,
            ResolveError::UnknownField {
                name: "DESCONHECIDO".to_owned(),
                location: "block 1".to_owned(),
            }
        );
        assert_eq!(
            error.to_string(),
            "unknown confidential field `{DESCONHECIDO}` at block 1"
        );
    }

    #[test]
    fn lowercase_and_literal_braces_stay_text() {
        let resolution = resolve_text("{nome} {{NOME}} {} { NOME }").expect("literal braces");
        let text = match resolution.document().blocks() {
            [Block::Paragraph(inlines)] => match inlines.as_slice() {
                [Inline::Text(text)] => text,
                other => panic!("expected text, found {other:?}"),
            },
            other => panic!("expected a paragraph, found {other:?}"),
        };
        assert_eq!(text, "{nome} {Fulano de Tal} {} { NOME }");
    }

    #[test]
    fn substituted_values_are_not_scanned_again() {
        let mut store = ConfidentialStore::new();
        store.insert(
            ConfidentialName::parse("NOME").expect("a name"),
            ConfidentialValue::new("{CPF}"),
        );
        store.insert(
            ConfidentialName::parse("CPF").expect("a name"),
            ConfidentialValue::new("000"),
        );
        let document = Document::new(
            Metadata::default(),
            vec![Block::Paragraph(vec![Inline::Text("{NOME}".to_owned())])],
        );
        let resolution = resolve(&document, &store).expect("resolution");
        let text = match resolution.document().blocks() {
            [Block::Paragraph(inlines)] => match inlines.as_slice() {
                [Inline::Text(text)] => text,
                other => panic!("expected text, found {other:?}"),
            },
            other => panic!("expected a paragraph, found {other:?}"),
        };
        assert_eq!(text, "{CPF}");
    }

    #[test]
    fn every_inline_container_is_scanned_but_code_and_destinations_are_not() {
        let document = Document::new(
            Metadata::default(),
            vec![
                Block::Heading(Heading::new(
                    HeadingLevel::new(1).expect("a level"),
                    vec![Inline::Text("{NOME}".to_owned())],
                )),
                Block::Paragraph(vec![
                    Inline::Emphasis(vec![Inline::Text("{NOME}".to_owned())]),
                    Inline::Strong(vec![Inline::Text("{CPF}".to_owned())]),
                    Inline::Strikethrough(vec![Inline::Text("{NOME}".to_owned())]),
                    Inline::Code("{NOME}".to_owned()),
                    Inline::Link {
                        destination: crate::document::Destination::new("{NOME}"),
                        title: Some("{NOME}".to_owned()),
                        content: vec![Inline::Text("{NOME}".to_owned())],
                    },
                    Inline::Image {
                        destination: crate::document::Destination::new("{NOME}"),
                        title: Some("{NOME}".to_owned()),
                        alternative: vec![Inline::Text("{NOME}".to_owned())],
                    },
                ]),
                Block::List(List::new(
                    ListStart::Unordered,
                    vec![ListItem::new(
                        Some(TaskState::Done),
                        vec![Block::Paragraph(vec![Inline::Text("{NOME}".to_owned())])],
                    )],
                    true,
                )),
                Block::Table(
                    Table::new(
                        vec![ColumnAlignment::Default],
                        vec![TableCell::new(vec![Inline::Text("{NOME}".to_owned())])],
                        vec![vec![TableCell::new(vec![Inline::Text("{CPF}".to_owned())])]],
                    )
                    .expect("a table"),
                ),
                Block::Quote(Quote::new(
                    crate::document::QuoteKind::Plain,
                    vec![Block::Paragraph(vec![Inline::Text("{NOME}".to_owned())])],
                )),
                Block::FootnoteDefinition(FootnoteDefinition::new(
                    crate::document::FootnoteId::parse("nota").expect("a footnote"),
                    vec![Block::Paragraph(vec![Inline::Text("{NOME}".to_owned())])],
                )),
                Block::CodeBlock(crate::document::CodeBlock::new(None, "{NOME}".to_owned())),
            ],
        );
        let resolution = resolve(&document, &store()).expect("resolution");
        let source = format!("{:?}", resolution.document());
        assert!(source.contains("Fulano de Tal"));
        assert!(source.contains("000.000.000-00"));
        assert!(source.contains("{NOME}"));
        assert_eq!(resolution.names().len(), 2);
    }

    #[test]
    fn redaction_never_rescans_the_marker() {
        let mut store = ConfidentialStore::new();
        store.insert(
            ConfidentialName::parse("A").expect("a name"),
            ConfidentialValue::new("redacted"),
        );
        assert_eq!(store.redact("redacted e outro"), "[redacted] e outro");
        assert_eq!(store.redact("nada"), "nada");
    }

    #[test]
    fn the_store_debug_lists_names_only() {
        let store = store();
        let debug = format!("{store:?}");
        assert!(debug.contains("NOME"), "{debug}");
        assert!(!debug.contains("Fulano"), "{debug}");
    }

    #[test]
    fn the_first_field_is_found_for_frontmatter_checks() {
        assert_eq!(first_field("Nome {NOME}"), Some("NOME".to_owned()));
        assert_eq!(first_field("{nome}"), None);
        assert_eq!(first_field("sem campo"), None);
        assert_eq!(first_field("{A}{B}"), Some("A".to_owned()));
    }

    #[test]
    fn path_containment_resolves_dot_and_dot_dot() {
        let project = tempfile::TempDir::new().expect("a project");
        let root = project.path().join("source");
        std::fs::create_dir_all(&root).expect("the source must exist");
        assert!(is_within(&root, &root.join("nested").join("file.pdf")));
        assert!(!is_within(&root, &project.path().join("output")));
        assert!(!is_within(&root, &root.join("..").join("output")));
        assert!(is_within(
            &root,
            &root.join("a").join("..").join("b").join("file.pdf")
        ));
    }

    #[test]
    fn a_title_value_is_not_scanned() {
        let document = Document::new(
            Metadata::new(Some(Title::parse("{NOME}").expect("a title")), None, None),
            Vec::new(),
        );
        let resolution = resolve(&document, &store()).expect("resolution");
        assert_eq!(
            resolution.document().metadata().title().map(Title::as_str),
            Some("{NOME}")
        );
    }
}
