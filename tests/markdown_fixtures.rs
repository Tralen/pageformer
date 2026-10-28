//! Fixture based tests for the Markdown parser.

use pageformer::document::{
    Block, ColumnAlignment, Document, Heading, Inline, ListStart, Quote, Table,
};
use pageformer::markdown;

const CONTRACT: &str = include_str!("../assets/examples/escritorio/Contrato.md");
const EMBARGOS: &str = include_str!("../assets/examples/escritorio/Embargos.md");
const REPORT: &str = include_str!("../assets/examples/tecnico/Relatorio.md");

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
            _ => String::new(),
        })
        .collect()
}

fn headings(document: &Document) -> Vec<&Heading> {
    document
        .blocks()
        .iter()
        .filter_map(|block| match block {
            Block::Heading(heading) => Some(heading),
            _ => None,
        })
        .collect()
}

fn first_table(document: &Document) -> &Table {
    document
        .blocks()
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("a table")
}

fn first_quote(document: &Document) -> &Quote {
    document
        .blocks()
        .iter()
        .find_map(|block| match block {
            Block::Quote(quote) => Some(quote),
            _ => None,
        })
        .expect("a block quote")
}

#[test]
fn contract_forces_the_signature_block_to_a_new_page() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let index = document
        .blocks()
        .iter()
        .position(|block| matches!(block, Block::PageBreak))
        .expect("the contract has a page break");
    let next = document
        .blocks()
        .get(index + 1)
        .expect("a block after the break");
    match next {
        Block::Paragraph(inlines) => assert_eq!(plain_text(inlines), "Local e data:"),
        other => panic!("expected the signature block, found {other:?}"),
    }
}

#[test]
fn the_report_uses_the_new_template_features() {
    let document = markdown::parse(REPORT).expect("the report fixture must parse");
    let metadata = document.metadata();
    assert_eq!(
        metadata.date().map(|date| date.to_string()),
        Some("2026-03-10".to_owned())
    );
    assert_eq!(
        metadata.subject().map(|subject| subject.as_str()),
        Some("Demonstração das funcionalidades do template")
    );
    assert_eq!(metadata.keywords().len(), 2);
    assert!(
        document
            .blocks()
            .iter()
            .any(|block| matches!(block, Block::PageBreak)),
        "the report forces a page break"
    );
    assert!(
        document
            .blocks()
            .iter()
            .any(|block| matches!(block, Block::Table(_))),
        "the report has a table"
    );
}

#[test]
fn the_embargos_fixture_uses_confidential_fields() {
    for field in [
        "{JUIZO}",
        "{PROCESSO}",
        "{EMBARGANTE_RAZAO}",
        "{EMBARGANTE_CNPJ}",
        "{EMBARGANTE_ENDERECO}",
        "{SOCIO_CPF}",
        "{RECLAMADA_CNPJ}",
        "{ADVOGADO_1}",
        "{VALOR_CAUSA}",
    ] {
        assert!(EMBARGOS.contains(field), "the fixture is missing {field}");
    }
    for datum in ["55.444.333/0001-22", "SSS 999", "999.555.444-0"] {
        assert!(!EMBARGOS.contains(datum), "the fixture still has {datum}");
    }
}

#[test]
fn contract_frontmatter_is_read() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let metadata = document.metadata();
    assert_eq!(
        metadata.title().map(|title| title.as_str()),
        Some("Contrato de Prestação de Serviços")
    );
    assert_eq!(
        metadata.author().map(|author| author.as_str()),
        Some("Pageformer")
    );
    assert_eq!(
        metadata.language().map(|language| language.as_str()),
        Some("pt-BR")
    );
}

#[test]
fn contract_sections_are_level_two() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let headings = headings(&document);
    assert_eq!(headings.len(), 8);
    assert!(
        headings.iter().all(|heading| heading.level().get() == 2),
        "every contract section is a second level heading"
    );
    let first = headings.first().expect("a first heading");
    assert_eq!(plain_text(first.content()), "Das partes");
}

#[test]
fn contract_lists_keep_their_kind_and_items() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let list = document
        .blocks()
        .iter()
        .find_map(|block| match block {
            Block::List(list) => Some(list),
            _ => None,
        })
        .expect("the obligation list");
    assert_eq!(list.start(), ListStart::Unordered);
    assert_eq!(list.items().len(), 4);
    assert!(
        list.items().iter().all(|item| item.task().is_none()),
        "ordinary bullets have no task state"
    );
}

#[test]
fn contract_table_has_three_columns_and_three_rows() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let table = first_table(&document);
    assert_eq!(
        table.alignments(),
        &[
            ColumnAlignment::Default,
            ColumnAlignment::Default,
            ColumnAlignment::Default,
        ]
    );
    let header = table.header();
    assert_eq!(
        header.first().map(|cell| plain_text(cell.content())),
        Some("Parcela".to_owned())
    );
    assert_eq!(
        header.get(2).map(|cell| plain_text(cell.content())),
        Some("Valor".to_owned())
    );
    assert_eq!(table.rows().len(), 3);
    let first_row = table.rows().first().expect("a first row");
    assert_eq!(
        first_row.first().map(|cell| plain_text(cell.content())),
        Some("1".to_owned())
    );
}

#[test]
fn contract_quote_keeps_its_paragraphs() {
    let document = markdown::parse(CONTRACT).expect("the contract fixture must parse");
    let quote = first_quote(&document);
    assert_eq!(quote.blocks().len(), 2);
    let second = quote.blocks().get(1).expect("the second quote paragraph");
    let inlines = match second {
        Block::Paragraph(inlines) => inlines,
        other => panic!("expected a paragraph, found {other:?}"),
    };
    assert!(
        inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Emphasis(_)))
    );
}

#[test]
fn embargos_frontmatter_and_headings_are_read() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    assert_eq!(
        document.metadata().title().map(|title| title.as_str()),
        Some("EMBARGOS DE TERCEIRO")
    );
    assert_eq!(
        document.metadata().author().map(|author| author.as_str()),
        Some("Escritório Exemplo")
    );
    let headings = headings(&document);
    assert_eq!(headings.len(), 7);
    assert_eq!(
        headings
            .iter()
            .filter(|heading| heading.level().get() == 1)
            .count(),
        1
    );
    assert_eq!(
        headings
            .iter()
            .filter(|heading| heading.level().get() == 2)
            .count(),
        6
    );
}

#[test]
fn embargos_numbering_is_read_without_escapes() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let first = document
        .blocks()
        .iter()
        .find_map(|block| match block {
            Block::NumberedParagraph(paragraph) if paragraph.number() == 1 => Some(paragraph),
            _ => None,
        })
        .expect("numbered paragraph 1");
    assert!(plain_text(first.content()).starts_with("Requer"));
    assert!(
        !EMBARGOS.contains("\\."),
        "the fixture must not need escaped numbering"
    );
}

fn inlines_end_with_formatted_comma(inlines: &[Inline]) -> bool {
    inlines.iter().any(|inline| match inline {
        Inline::Strong(content) | Inline::Emphasis(content) => {
            plain_text(content).trim_end().ends_with(',')
                || inlines_end_with_formatted_comma(content)
        }
        Inline::Strikethrough(content) => inlines_end_with_formatted_comma(content),
        Inline::Link { content, .. } => inlines_end_with_formatted_comma(content),
        Inline::Image { alternative, .. } => inlines_end_with_formatted_comma(alternative),
        _ => false,
    })
}

fn blocks_have_formatted_comma(blocks: &[Block]) -> bool {
    blocks.iter().any(|block| match block {
        Block::Heading(heading) => inlines_end_with_formatted_comma(heading.content()),
        Block::Paragraph(inlines) => inlines_end_with_formatted_comma(inlines),
        Block::NumberedParagraph(paragraph) => {
            inlines_end_with_formatted_comma(paragraph.content())
        }
        Block::List(list) => list
            .items()
            .iter()
            .any(|item| blocks_have_formatted_comma(item.blocks())),
        Block::Table(table) => table
            .header()
            .iter()
            .chain(table.rows().iter().flatten())
            .any(|cell| inlines_end_with_formatted_comma(cell.content())),
        Block::Quote(quote) => blocks_have_formatted_comma(quote.blocks()),
        Block::FootnoteDefinition(definition) => blocks_have_formatted_comma(definition.blocks()),
        Block::CodeBlock(_) | Block::ThematicBreak => false,
        _ => false,
    })
}

#[test]
fn embargos_commas_are_outside_inline_formatting() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    assert!(
        !blocks_have_formatted_comma(document.blocks()),
        "a formatted span ends with a comma"
    );
}

#[test]
fn embargos_final_table_has_two_columns() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let table = first_table(&document);
    assert_eq!(table.header().len(), 2);
    assert_eq!(
        table
            .header()
            .first()
            .map(|cell| plain_text(cell.content())),
        Some("{ADVOGADO_1}".to_owned())
    );
    let row = table.rows().first().expect("a signature row");
    assert_eq!(
        row.first().map(|cell| plain_text(cell.content())),
        Some("{OAB_1}".to_owned())
    );
    assert_eq!(table.rows().len(), 1);
}

#[test]
fn embargos_emphasis_and_strong_are_preserved() {
    let document = markdown::parse(EMBARGOS).expect("the embargos fixture must parse");
    let has_strong = document.blocks().iter().any(|block| match block {
        Block::Paragraph(inlines) => inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Strong(_))),
        _ => false,
    });
    let has_emphasis = document.blocks().iter().any(|block| match block {
        Block::Paragraph(inlines) => inlines
            .iter()
            .any(|inline| matches!(inline, Inline::Emphasis(_))),
        _ => false,
    });
    assert!(has_strong, "the fixture uses bold text");
    assert!(has_emphasis, "the fixture uses italic text");
}
