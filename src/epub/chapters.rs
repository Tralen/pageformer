//! Splits the document into EPUB chapters.

use crate::document::Block;
use crate::html::content::plain_text;

/// How a chapter is labelled in the navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChapterKind {
    Introduction,
    Heading(String),
    WholeDocument,
}

/// One chapter before rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chapter {
    pub kind: ChapterKind,
    pub blocks: Vec<Block>,
}

/// Splits the blocks at each first-level heading.
///
/// Content before the first heading becomes an introduction. A document
/// without a first-level heading remains one whole-document chapter. The
/// renderer drops a chapter whose body is empty, so a block list with only
/// definitions does not become an empty file.
pub(crate) fn split(blocks: &[Block]) -> Vec<Chapter> {
    if blocks.is_empty() {
        return Vec::new();
    }
    if !blocks.iter().any(is_first_level) {
        return vec![Chapter {
            kind: ChapterKind::WholeDocument,
            blocks: blocks.to_vec(),
        }];
    }

    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for block in blocks {
        if is_first_level(block) && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        current.push(block.clone());
    }
    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
        .into_iter()
        .map(|blocks| {
            let kind = match blocks.first() {
                Some(Block::Heading(heading)) if heading.level().get() == 1 => {
                    let label = plain_text(heading.content());
                    if label.is_empty() {
                        ChapterKind::Heading("Untitled chapter".to_owned())
                    } else {
                        ChapterKind::Heading(label)
                    }
                }
                _ => ChapterKind::Introduction,
            };
            Chapter { kind, blocks }
        })
        .collect()
}

fn is_first_level(block: &Block) -> bool {
    matches!(block, Block::Heading(heading) if heading.level().get() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown;

    fn labels(source: &str) -> Vec<ChapterKind> {
        let document = markdown::parse(source).expect("the document must parse");
        split(document.blocks())
            .into_iter()
            .map(|chapter| chapter.kind)
            .collect()
    }

    #[test]
    fn a_first_level_heading_starts_a_chapter() {
        assert_eq!(
            labels("Introdução.\n\n# Um\n\nTexto.\n\n# Dois\n\nTexto.\n"),
            vec![
                ChapterKind::Introduction,
                ChapterKind::Heading("Um".to_owned()),
                ChapterKind::Heading("Dois".to_owned()),
            ]
        );
    }

    #[test]
    fn a_document_without_a_first_level_heading_is_one_chapter() {
        assert_eq!(
            labels("## Seção\n\nTexto.\n"),
            vec![ChapterKind::WholeDocument]
        );
    }

    #[test]
    fn an_empty_document_has_no_chapters() {
        assert!(labels("").is_empty());
    }
}
