//! Output format selection.

use std::fmt;
use std::str::FromStr;

/// The output formats the application can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Format {
    Pdf,
    Txt,
    Odt,
    Docx,
    Html,
    Epub,
}

impl Format {
    /// Every format the library can render today.
    pub const ALL: [Self; 6] = [
        Self::Pdf,
        Self::Txt,
        Self::Odt,
        Self::Docx,
        Self::Html,
        Self::Epub,
    ];

    /// The name used on the command line and in the format menu.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Txt => "txt",
            Self::Odt => "odt",
            Self::Docx => "docx",
            Self::Html => "html",
            Self::Epub => "epub",
        }
    }

    /// The file extension of the output.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Txt => "txt",
            Self::Odt => "odt",
            Self::Docx => "docx",
            Self::Html => "html",
            Self::Epub => "epub",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

impl FromStr for Format {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "pdf" => Ok(Self::Pdf),
            "txt" => Ok(Self::Txt),
            "odt" => Ok(Self::Odt),
            "docx" => Ok(Self::Docx),
            "html" => Ok(Self::Html),
            "epub" => Ok(Self::Epub),
            other => Err(FormatError::Unknown(other.to_owned())),
        }
    }
}

/// Why a format name was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    Unknown(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(name) => {
                write!(
                    formatter,
                    "unknown format `{name}`; use pdf, txt, odt, docx, html or epub"
                )
            }
        }
    }
}

impl std::error::Error for FormatError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_name_selects_the_extension() {
        assert_eq!(Format::Pdf.extension(), "pdf");
        assert_eq!(Format::Txt.extension(), "txt");
        assert_eq!(Format::Odt.extension(), "odt");
        assert_eq!(Format::Docx.extension(), "docx");
        assert_eq!(Format::Html.extension(), "html");
        assert_eq!(Format::Epub.extension(), "epub");
        assert_eq!("txt".parse::<Format>(), Ok(Format::Txt));
        assert_eq!("odt".parse::<Format>(), Ok(Format::Odt));
        assert_eq!("docx".parse::<Format>(), Ok(Format::Docx));
        assert_eq!("html".parse::<Format>(), Ok(Format::Html));
        assert_eq!("epub".parse::<Format>(), Ok(Format::Epub));
    }

    #[test]
    fn an_unknown_format_name_is_rejected() {
        let error = "mobi".parse::<Format>().expect_err("an unknown format");

        assert!(
            error
                .to_string()
                .contains("pdf, txt, odt, docx, html or epub"),
            "{error}"
        );
    }

    #[test]
    fn every_supported_format_has_a_name_and_an_extension() {
        for format in Format::ALL {
            assert_eq!(format.to_string(), format.name());
            assert_eq!(format.extension(), format.name());
        }
    }
}
