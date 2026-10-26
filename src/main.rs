//! Generates a PDF/A-2b sample with Typst to measure the engine cost.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards};

const MAIN_PATH: &str = "spike.typ";
const SOURCE: &str = include_str!("../assets/spike.typ");
const OUTPUT: &str = ".examples/Contrato.pdf";
const FIXED_YEAR: i32 = 2026;
const FIXED_MONTH: u8 = 1;
const FIXED_DAY: u8 = 1;

fn main() {
    match run() {
        Ok(()) => println!("wrote {OUTPUT}"),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), SpikeError> {
    let world = SpikeWorld::new()?;
    let warned = typst::compile::<PagedDocument>(&world);
    report_warnings(&world, &warned.warnings);
    let document = warned
        .output
        .map_err(|errors| SpikeError::Compilation(format_diagnostics(&world, &errors)))?;

    let standards = PdfStandards::new(&[PdfStandard::A_2b])
        .map_err(|error| SpikeError::Standards(error.message().to_string()))?;
    let options = PdfOptions {
        standards,
        ..PdfOptions::default()
    };
    let pdf = typst_pdf::pdf(&document, &options)
        .map_err(|errors| SpikeError::Pdf(format_diagnostics(&world, &errors)))?;
    write_atomic(Path::new(OUTPUT), &pdf)?;
    Ok(())
}

fn report_warnings(world: &SpikeWorld, warnings: &[SourceDiagnostic]) {
    for warning in warnings {
        eprintln!("warning: {}", format_diagnostic(world, warning));
    }
}

fn format_diagnostic(world: &SpikeWorld, diagnostic: &SourceDiagnostic) -> String {
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

fn format_diagnostics(world: &SpikeWorld, diagnostics: &[SourceDiagnostic]) -> String {
    if diagnostics.is_empty() {
        return "unknown error".to_owned();
    }
    diagnostics
        .iter()
        .map(|diagnostic| format_diagnostic(world, diagnostic))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("pdf.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)
}

struct SpikeWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    source: Source,
}

impl SpikeWorld {
    fn new() -> Result<Self, SpikeError> {
        let virtual_path = VirtualPath::new(MAIN_PATH).map_err(|_| SpikeError::InvalidPath)?;
        let main = FileId::unique(RootedPath::new(VirtualRoot::Project, virtual_path));
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::system());
        Ok(Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main,
            source: Source::new(main, SOURCE.to_owned()),
        })
    }
}

impl World for SpikeWorld {
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
            Ok(Bytes::from_string(SOURCE.to_owned()))
        } else {
            Err(FileError::NotFound(MAIN_PATH.into()))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        Datetime::from_ymd(FIXED_YEAR, FIXED_MONTH, FIXED_DAY)
    }
}

#[derive(Debug)]
enum SpikeError {
    InvalidPath,
    Compilation(String),
    Standards(String),
    Pdf(String),
    Io(io::Error),
}

impl fmt::Display for SpikeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => formatter.write_str("invalid virtual path"),
            Self::Compilation(message) => write!(formatter, "compilation failed: {message}"),
            Self::Standards(message) => write!(formatter, "invalid PDF standard: {message}"),
            Self::Pdf(message) => write!(formatter, "PDF export failed: {message}"),
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
        }
    }
}

impl Error for SpikeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SpikeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
