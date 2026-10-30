//! Plain-text backend.
//!
//! The output is UTF-8 with LF line endings, no automatic wrapping and one
//! blank line between blocks. Layout is ignored: headings become their text,
//! lists keep a marker, tables become tab-separated rows with a separator row
//! and an explicit page break becomes a form feed.

use std::convert::Infallible;

use crate::document::{
    Block, Document, FootnoteDefinition, Inline, List, ListStart, NumberedParagraph, Quote, Table,
    TableCell, TaskState,
};
use crate::render::{Rendered, Renderer};
use crate::template::Template;

/// The form feed that marks an explicit page break.
pub const PAGE_BREAK: &str = "\u{c}";

/// The plain-text backend. It ignores layout and cannot fail.
#[derive(Debug, Clone, Copy, Default)]
pub struct TxtRenderer;

impl Renderer for TxtRenderer {
    type Error = Infallible;

    fn render(&self, document: &Document, _template: &Template) -> Result<Rendered, Self::Error> {
        Ok(Rendered::new(text(document).into_bytes(), Vec::new()))
    }
}

/// Renders a resolved document as plain text.
pub fn text(document: &Document) -> String {
    let mut footnotes = Vec::new();
    let body = blocks(document.blocks(), "\n\n", &mut footnotes);
    let mut sections = Vec::new();
    if !body.is_empty() {
        sections.push(body);
    }
    if !footnotes.is_empty() {
        sections.push(footnotes.join("\n\n"));
    }
    let joined = sections.join("\n\n");
    if joined.is_empty() {
        String::new()
    } else {
        format!("{joined}\n")
    }
}

fn blocks(blocks: &[Block], separator: &str, footnotes: &mut Vec<String>) -> String {
    let mut rendered = Vec::new();
    for block in blocks {
        let text = match block {
            Block::FootnoteDefinition(definition) => {
                let definition = footnote(definition, footnotes);
                footnotes.push(definition);
                continue;
            }
            _ => block_text(block, footnotes),
        };
        if !text.is_empty() {
            rendered.push(text);
        }
    }
    rendered.join(separator)
}

fn block_text(block: &Block, footnotes: &mut Vec<String>) -> String {
    match block {
        Block::Heading(heading) => inline(heading.content()),
        Block::Paragraph(content) => inline(content),
        Block::NumberedParagraph(paragraph) => numbered(paragraph),
        Block::List(value) => list(value, footnotes),
        Block::Table(value) => table(value),
        Block::Quote(value) => quote(value, footnotes),
        Block::CodeBlock(code) => code.text().trim_end_matches('\n').to_owned(),
        Block::ThematicBreak => "---".to_owned(),
        Block::PageBreak => PAGE_BREAK.to_owned(),
        Block::FootnoteDefinition(_) => String::new(),
    }
}

fn numbered(paragraph: &NumberedParagraph) -> String {
    format!("{}. {}", paragraph.number(), inline(paragraph.content()))
}

fn list(list: &List, footnotes: &mut Vec<String>) -> String {
    let separator = if list.is_tight() { "\n" } else { "\n\n" };
    let mut rendered = Vec::new();
    let mut number = match list.start() {
        ListStart::Ordered(start) => start,
        ListStart::Unordered => 1,
    };
    for item in list.items() {
        let marker = marker(list.start(), item.task(), number);
        let content = blocks(item.blocks(), separator, footnotes);
        if content.is_empty() {
            rendered.push(marker.trim_end().to_owned());
        } else {
            rendered.push(format!(
                "{marker}{}",
                indent_after_first(&content, marker.len())
            ));
        }
        if matches!(list.start(), ListStart::Ordered(_)) {
            number = number.saturating_add(1);
        }
    }
    rendered.join(separator)
}

fn marker(start: ListStart, task: Option<TaskState>, number: u64) -> String {
    let bullet = match start {
        ListStart::Unordered => "-".to_owned(),
        ListStart::Ordered(_) => format!("{number}."),
    };
    match task {
        None => format!("{bullet} "),
        Some(TaskState::Todo) => format!("{bullet} [ ] "),
        Some(TaskState::Done) => format!("{bullet} [x] "),
    }
}

fn table(table: &Table) -> String {
    let header = row(table.header());
    let separator = vec!["---"; table.alignments().len()].join("\t");
    let mut rows = vec![header, separator];
    for row_cells in table.rows() {
        rows.push(row(row_cells));
    }
    rows.join("\n")
}

fn row(cells: &[TableCell]) -> String {
    cells
        .iter()
        .map(|cell| inline(cell.content()).replace('\n', " "))
        .collect::<Vec<_>>()
        .join("\t")
}

fn quote(quote: &Quote, footnotes: &mut Vec<String>) -> String {
    blocks(quote.blocks(), "\n\n", footnotes)
        .lines()
        .map(|line| {
            if line.is_empty() {
                ">".to_owned()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn footnote(definition: &FootnoteDefinition, footnotes: &mut Vec<String>) -> String {
    let content = blocks(definition.blocks(), "\n\n", footnotes);
    let id = definition.id().as_str();
    if content.is_empty() {
        format!("[^{id}]")
    } else {
        format!("[^{id}] {}", indent_after_first(&content, id.len() + 4))
    }
}

fn indent_after_first(text: &str, width: usize) -> String {
    let padding = " ".repeat(width);
    let mut lines = text.split('\n');
    let mut output = String::new();
    if let Some(first) = lines.next() {
        output.push_str(first);
    }
    for line in lines {
        output.push('\n');
        if !line.is_empty() {
            output.push_str(&padding);
            output.push_str(line);
        }
    }
    output
}

fn inline(inlines: &[Inline]) -> String {
    let mut output = String::new();
    for item in inlines {
        match item {
            Inline::Text(text) | Inline::Code(text) => output.push_str(text),
            Inline::Emphasis(content)
            | Inline::Strong(content)
            | Inline::Strikethrough(content) => output.push_str(&inline(content)),
            Inline::Link {
                destination,
                content,
                ..
            } => {
                let label = inline(content);
                if label.is_empty() {
                    output.push_str(destination.as_str());
                } else {
                    output.push_str(&format!("{label} ({})", destination.as_str()));
                }
            }
            Inline::Image { alternative, .. } => {
                output.push('[');
                output.push_str(&inline(alternative));
                output.push(']');
            }
            Inline::SoftBreak => output.push(' '),
            Inline::HardBreak => output.push('\n'),
            Inline::FootnoteReference(id) => {
                output.push_str(&format!("[^{}]", id.as_str()));
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Metadata, NumberedParagraph};
    use crate::markdown;

    fn rendered(source: &str) -> String {
        let document = markdown::parse(source).expect("the document must parse");
        text(&document)
    }

    #[test]
    fn an_empty_document_is_an_empty_string() {
        let document = Document::new(Metadata::default(), Vec::new());

        assert_eq!(text(&document), "");
    }

    #[test]
    fn headings_are_their_text_and_blocks_are_separated_by_a_blank_line() {
        assert_eq!(
            rendered("# Título\n\nUm parágrafo.\n\n## Seção\n\nOutro."),
            "Título\n\nUm parágrafo.\n\nSeção\n\nOutro.\n"
        );
    }

    #[test]
    fn numbered_paragraphs_keep_their_number() {
        let document = Document::new(
            Metadata::default(),
            vec![Block::NumberedParagraph(NumberedParagraph::new(
                3,
                vec![Inline::Text("Cláusula.".to_owned())],
            ))],
        );

        assert_eq!(text(&document), "3. Cláusula.\n");
    }

    #[test]
    fn unordered_and_ordered_lists_keep_their_markers() {
        assert_eq!(rendered("- um\n- dois\n"), "- um\n- dois\n");
        assert_eq!(rendered("3. três\n4. quatro\n"), "3. três\n4. quatro\n");
    }

    #[test]
    fn nested_lists_are_indented_under_the_parent_marker() {
        assert_eq!(
            rendered("- um\n- dois\n  - fundo\n"),
            "- um\n- dois\n  - fundo\n"
        );
    }

    #[test]
    fn task_items_keep_their_state() {
        assert_eq!(
            rendered("- [ ] aberto\n- [x] feito\n"),
            "- [ ] aberto\n- [x] feito\n"
        );
    }

    #[test]
    fn tables_use_tabs_and_a_separator_row() {
        assert_eq!(
            rendered("| A | B |\n|---|---|\n| 1 | 2 |\n"),
            "A\tB\n---\t---\n1\t2\n"
        );
    }

    #[test]
    fn quotes_are_prefixed_on_every_line() {
        assert_eq!(
            rendered("> uma linha\n>\n> outra linha\n"),
            "> uma linha\n>\n> outra linha\n"
        );
    }

    #[test]
    fn code_blocks_are_preserved() {
        assert_eq!(rendered("```\nlet x = 1;\n```\n"), "let x = 1;\n");
    }

    #[test]
    fn links_and_images_keep_a_readable_form() {
        assert_eq!(
            rendered("Veja [o site](https://example.org) e ![logo](logo.png)."),
            "Veja o site (https://example.org) e [logo].\n"
        );
    }

    #[test]
    fn soft_breaks_become_spaces_and_hard_breaks_become_newlines() {
        assert_eq!(rendered("uma\nlinha\n"), "uma linha\n");
        assert_eq!(rendered("uma  \nlinha\n"), "uma\nlinha\n");
    }

    #[test]
    fn footnotes_are_collected_at_the_end() {
        assert_eq!(
            rendered("Texto[^1].\n\n[^1]: Nota.\n"),
            "Texto[^1].\n\n[^1] Nota.\n"
        );
    }

    #[test]
    fn an_explicit_page_break_becomes_a_form_feed() {
        assert_eq!(
            rendered("Antes.\n\n{pagebreak}\n\nDepois.\n"),
            "Antes.\n\n\u{c}\n\nDepois.\n"
        );
    }

    #[test]
    fn a_thematic_break_is_three_dashes() {
        assert_eq!(
            rendered("Antes.\n\n---\n\nDepois.\n"),
            "Antes.\n\n---\n\nDepois.\n"
        );
    }
}
