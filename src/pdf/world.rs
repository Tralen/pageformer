//! The minimal Typst world used by the PDF backend.

use std::collections::BTreeMap;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::fonts::FontStore;

use crate::document::DocumentDate;

use super::PdfError;

const MAIN_PATH: &str = "document.typ";

pub(crate) struct PdfWorld<'a> {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source: Source,
    files: &'a BTreeMap<String, Vec<u8>>,
    date: Datetime,
}

impl<'a> PdfWorld<'a> {
    pub(crate) fn new(
        source: String,
        files: &'a BTreeMap<String, Vec<u8>>,
        date: DocumentDate,
    ) -> Result<Self, PdfError> {
        let datetime = Datetime::from_ymd(date.year(), date.month(), date.day())
            .ok_or(PdfError::InvalidDate)?;
        let virtual_path = VirtualPath::new(MAIN_PATH).map_err(|_| PdfError::InvalidPath)?;
        let main = FileId::unique(RootedPath::new(VirtualRoot::Project, virtual_path));
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
        Ok(Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            source: Source::new(main, source),
            files,
            date: datetime,
        })
    }
}

impl World for PdfWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(MAIN_PATH.into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            return Ok(Bytes::from_string(self.source.text().to_owned()));
        }
        let path = id.vpath().get_without_slash();
        self.files
            .get(path)
            .cloned()
            .map(Bytes::new)
            .ok_or_else(|| FileError::NotFound(path.into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        Some(self.date)
    }
}

pub(crate) fn format_diagnostic(world: &PdfWorld<'_>, diagnostic: &SourceDiagnostic) -> String {
    let position = world
        .range(diagnostic.span)
        .and_then(|range| world.source.lines().byte_to_line_column(range.start));
    match position {
        Some((line, column)) => format!(
            "{} (line {}, column {})",
            diagnostic.message,
            line + 1,
            column + 1
        ),
        None => diagnostic.message.to_string(),
    }
}

pub(crate) fn format_diagnostics(world: &PdfWorld<'_>, diagnostics: &[SourceDiagnostic]) -> String {
    if diagnostics.is_empty() {
        return "unknown error".to_owned();
    }
    diagnostics
        .iter()
        .map(|diagnostic| format_diagnostic(world, diagnostic))
        .collect::<Vec<_>>()
        .join(" | ")
}
