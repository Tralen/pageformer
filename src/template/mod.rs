//! The generic YAML template.
//!
//! One template drives every output format, so every field is semantic and none
//! of them depends on a backend. The nearest local template wins entirely: there
//! is no merge, and an invalid template is fatal.

pub mod loader;

use std::fmt;

use serde::Deserialize;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, Deserializer, MapAccess, Unexpected, Visitor};

const MM_TO_POINTS: f64 = 72.0 / 25.4;

/// The current schema version. A template for another version is rejected.
pub const CURRENT_SCHEMA_VERSION: u16 = 4;

const DEFAULT_MARGIN: Length = Length(20.0 * MM_TO_POINTS);
const DEFAULT_FONT_SIZE: Length = Length(12.0);
const DEFAULT_TITLE_SIZE: Length = Length(18.0);
const DEFAULT_LEADING: Length = Length(7.8);

const A4_DIMENSIONS: PageDimensions = PageDimensions {
    width: Length(210.0 * MM_TO_POINTS),
    height: Length(297.0 * MM_TO_POINTS),
};
const A5_DIMENSIONS: PageDimensions = PageDimensions {
    width: Length(148.0 * MM_TO_POINTS),
    height: Length(210.0 * MM_TO_POINTS),
};
const LETTER_DIMENSIONS: PageDimensions = PageDimensions {
    width: Length(8.5 * 72.0),
    height: Length(11.0 * 72.0),
};
const LEGAL_DIMENSIONS: PageDimensions = PageDimensions {
    width: Length(8.5 * 72.0),
    height: Length(14.0 * 72.0),
};
const OFICIO_DIMENSIONS: PageDimensions = PageDimensions {
    width: Length(216.0 * MM_TO_POINTS),
    height: Length(330.0 * MM_TO_POINTS),
};

/// A validated template.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    version: SchemaVersion,
    page: Page,
    text: TextStyle,
}

impl Template {
    /// Parses and validates a template written as YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self, TemplateError> {
        let probe: VersionProbe =
            serde_yaml::from_str(yaml).map_err(|error| TemplateError::Yaml {
                message: error.to_string(),
            })?;
        if probe.version != CURRENT_SCHEMA_VERSION {
            return Err(TemplateError::UnsupportedVersion {
                found: probe.version,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }
        let raw: RawTemplate = serde_yaml::from_str(yaml).map_err(|error| TemplateError::Yaml {
            message: error.to_string(),
        })?;
        Self::from_raw(raw)
    }

    /// Parses the template embedded in the binary.
    pub fn embedded() -> Result<Self, TemplateError> {
        Self::from_yaml(include_str!("../../assets/default.pageformer.yaml"))
    }

    fn from_raw(raw: RawTemplate) -> Result<Self, TemplateError> {
        if raw.version != CURRENT_SCHEMA_VERSION {
            return Err(TemplateError::UnsupportedVersion {
                found: raw.version,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(Self {
            version: SchemaVersion::CURRENT,
            page: convert_page(raw.page)?,
            text: convert_text(raw.text)?,
        })
    }

    pub fn schema_version(&self) -> SchemaVersion {
        self.version
    }

    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn text(&self) -> &TextStyle {
        &self.text
    }
}

/// The version of the template schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchemaVersion(u16);

impl SchemaVersion {
    pub const CURRENT: Self = Self(CURRENT_SCHEMA_VERSION);

    pub fn get(self) -> u16 {
        self.0
    }
}

/// Why a template was rejected.
#[derive(Debug)]
pub enum TemplateError {
    Yaml {
        message: String,
    },
    UnsupportedVersion {
        found: u16,
        supported: u16,
    },
    Value {
        field: &'static str,
        problem: ValueError,
    },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml { message } => write!(formatter, "invalid template: {message}"),
            Self::UnsupportedVersion { found, supported } => write!(
                formatter,
                "unsupported template schema version {found}; this build supports version {supported}"
            ),
            Self::Value { field, problem } => {
                write!(formatter, "invalid value for `{field}`: {problem}")
            }
        }
    }
}

impl std::error::Error for TemplateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yaml { .. } | Self::UnsupportedVersion { .. } | Self::Value { .. } => None,
        }
    }
}

/// Why a template value was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueError {
    EmptyText,
    InvalidNumber,
    UnknownUnit,
    NonFinite,
    Negative,
    Zero,
    EmptyList,
}

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyText => "the text is empty",
            Self::InvalidNumber => "the number is not valid",
            Self::UnknownUnit => "the unit is not supported; use mm, cm, in or pt",
            Self::NonFinite => "the number must be finite",
            Self::Negative => "the number must not be negative",
            Self::Zero => "the number must be greater than zero",
            Self::EmptyList => "the list must not be empty",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ValueError {}

/// A geometric length stored in typographic points.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Length(f64);

impl Length {
    pub fn new(points: f64) -> Result<Self, ValueError> {
        if !points.is_finite() {
            Err(ValueError::NonFinite)
        } else if points < 0.0 {
            Err(ValueError::Negative)
        } else {
            Ok(Self(points))
        }
    }

    /// Parses a length such as `30mm`, `2.5cm`, `1in` or `12pt`.
    /// A number without a unit is in points.
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let trimmed = text.trim();
        let (number, factor) = if let Some(number) = trimmed.strip_suffix("mm") {
            (number, MM_TO_POINTS)
        } else if let Some(number) = trimmed.strip_suffix("cm") {
            (number, 10.0 * MM_TO_POINTS)
        } else if let Some(number) = trimmed.strip_suffix("in") {
            (number, 72.0)
        } else if let Some(number) = trimmed.strip_suffix("pt") {
            (number, 1.0)
        } else if trimmed.ends_with(|character: char| character.is_ascii_alphabetic()) {
            return Err(ValueError::UnknownUnit);
        } else {
            (trimmed, 1.0)
        };
        let value: f64 = number
            .trim()
            .parse()
            .map_err(|_| ValueError::InvalidNumber)?;
        Self::new(value * factor)
    }

    pub fn points(self) -> f64 {
        self.0
    }

    pub fn millimetres(self) -> f64 {
        self.0 / MM_TO_POINTS
    }
}

/// The four page margins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    top: Length,
    bottom: Length,
    left: Length,
    right: Length,
}

impl Margins {
    pub fn new(top: Length, bottom: Length, left: Length, right: Length) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
        }
    }

    pub fn all(length: Length) -> Self {
        Self::new(length, length, length, length)
    }

    pub fn top(&self) -> Length {
        self.top
    }

    pub fn bottom(&self) -> Length {
        self.bottom
    }

    pub fn left(&self) -> Length {
        self.left
    }

    pub fn right(&self) -> Length {
        self.right
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self::all(DEFAULT_MARGIN)
    }
}

/// The paper size before the orientation is applied.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PageSize {
    A4,
    A5,
    Letter,
    Legal,
    Oficio,
    Custom(PageDimensions),
}

impl PageSize {
    pub fn dimensions(self) -> PageDimensions {
        match self {
            Self::A4 => A4_DIMENSIONS,
            Self::A5 => A5_DIMENSIONS,
            Self::Letter => LETTER_DIMENSIONS,
            Self::Legal => LEGAL_DIMENSIONS,
            Self::Oficio => OFICIO_DIMENSIONS,
            Self::Custom(dimensions) => dimensions,
        }
    }
}

impl<'de> Deserialize<'de> for PageSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PageSizeVisitor;

        impl<'de> Visitor<'de> for PageSizeVisitor {
            type Value = PageSize;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .write_str("a page size name such as `a4`, or a map with `width` and `height`")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    "a4" => Ok(PageSize::A4),
                    "a5" => Ok(PageSize::A5),
                    "letter" => Ok(PageSize::Letter),
                    "legal" => Ok(PageSize::Legal),
                    "oficio" => Ok(PageSize::Oficio),
                    other => Err(E::invalid_value(
                        Unexpected::Str(other),
                        &"a4, a5, letter, legal or oficio",
                    )),
                }
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let input = DimensionsInput::deserialize(MapAccessDeserializer::new(map))?;
                let width = Length::parse(&input.width)
                    .map_err(|problem| de::Error::custom(format!("invalid `width`: {problem}")))?;
                let height = Length::parse(&input.height)
                    .map_err(|problem| de::Error::custom(format!("invalid `height`: {problem}")))?;
                Ok(PageSize::Custom(PageDimensions { width, height }))
            }
        }

        deserializer.deserialize_any(PageSizeVisitor)
    }
}

/// The width and height of a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageDimensions {
    width: Length,
    height: Length,
}

impl PageDimensions {
    pub fn new(width: Length, height: Length) -> Self {
        Self { width, height }
    }

    pub fn width(self) -> Length {
        self.width
    }

    pub fn height(self) -> Length {
        self.height
    }
}

/// Whether the page is taller than it is wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// The page configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    size: PageSize,
    orientation: Orientation,
    margins: Margins,
    letterhead: Option<TemplatePath>,
}

impl Page {
    pub fn new(
        size: PageSize,
        orientation: Orientation,
        margins: Margins,
        letterhead: Option<TemplatePath>,
    ) -> Self {
        Self {
            size,
            orientation,
            margins,
            letterhead,
        }
    }

    pub fn size(&self) -> PageSize {
        self.size
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    pub fn margins(&self) -> Margins {
        self.margins
    }

    pub fn letterhead(&self) -> Option<&TemplatePath> {
        self.letterhead.as_ref()
    }

    /// The final page size, with the orientation applied.
    pub fn dimensions(&self) -> PageDimensions {
        let dimensions = self.size.dimensions();
        match self.orientation {
            Orientation::Portrait => dimensions,
            Orientation::Landscape => PageDimensions {
                width: dimensions.height,
                height: dimensions.width,
            },
        }
    }
}

/// A path written in the template, relative to the template file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplatePath(String);

impl TemplatePath {
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(ValueError::EmptyText)
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The font families and size for one kind of text.
#[derive(Debug, Clone, PartialEq)]
pub struct FontStyle {
    font_families: Vec<FontFamily>,
    size: Length,
}

impl FontStyle {
    pub fn new(font_families: Vec<FontFamily>, size: Length) -> Result<Self, ValueError> {
        if font_families.is_empty() {
            return Err(ValueError::EmptyList);
        }
        if size.points() <= 0.0 {
            return Err(ValueError::Zero);
        }
        Ok(Self {
            font_families,
            size,
        })
    }

    pub fn font_families(&self) -> &[FontFamily] {
        &self.font_families
    }

    pub fn size(&self) -> Length {
        self.size
    }
}

/// The text styling shared by every backend.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    body: FontStyle,
    title: FontStyle,
    leading: Length,
    paragraph_spacing: Option<Length>,
    header_spacing: Option<Length>,
    alignment: ParagraphAlignment,
}

impl TextStyle {
    pub fn new(
        body: FontStyle,
        title: FontStyle,
        leading: Length,
        paragraph_spacing: Option<Length>,
        header_spacing: Option<Length>,
        alignment: ParagraphAlignment,
    ) -> Self {
        Self {
            body,
            title,
            leading,
            paragraph_spacing,
            header_spacing,
            alignment,
        }
    }

    pub fn body(&self) -> &FontStyle {
        &self.body
    }

    pub fn title(&self) -> &FontStyle {
        &self.title
    }

    pub fn leading(&self) -> Length {
        self.leading
    }

    pub fn paragraph_spacing(&self) -> Option<Length> {
        self.paragraph_spacing
    }

    /// The space above a level-1 heading.
    pub fn header_spacing(&self) -> Option<Length> {
        self.header_spacing
    }

    pub fn alignment(&self) -> ParagraphAlignment {
        self.alignment
    }
}

/// One font family name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily(String);

impl FontFamily {
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Err(ValueError::EmptyText)
        } else {
            Ok(Self(trimmed.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How paragraphs are aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParagraphAlignment {
    Justified,
    Left,
}

#[derive(Debug, Deserialize)]
struct VersionProbe {
    version: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplate {
    version: u16,
    #[serde(default)]
    page: RawPage,
    #[serde(default)]
    text: RawText,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    size: Option<PageSize>,
    orientation: Option<Orientation>,
    margins: Option<RawMargins>,
    letterhead: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMargins {
    top: Option<String>,
    bottom: Option<String>,
    left: Option<String>,
    right: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawText {
    body: Option<RawFontStyle>,
    title: Option<RawFontStyle>,
    leading: Option<String>,
    paragraph_spacing: Option<String>,
    header_spacing: Option<String>,
    alignment: Option<ParagraphAlignment>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFontStyle {
    font_families: Option<Vec<String>>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DimensionsInput {
    width: String,
    height: String,
}

fn convert_page(raw: RawPage) -> Result<Page, TemplateError> {
    let margins = match raw.margins {
        Some(margins) => Margins::new(
            margin(margins.top, "page.margins.top")?.unwrap_or(DEFAULT_MARGIN),
            margin(margins.bottom, "page.margins.bottom")?.unwrap_or(DEFAULT_MARGIN),
            margin(margins.left, "page.margins.left")?.unwrap_or(DEFAULT_MARGIN),
            margin(margins.right, "page.margins.right")?.unwrap_or(DEFAULT_MARGIN),
        ),
        None => Margins::default(),
    };
    let letterhead = raw
        .letterhead
        .map(|text| TemplatePath::parse(&text))
        .transpose()
        .map_err(|problem| TemplateError::Value {
            field: "page.letterhead",
            problem,
        })?;
    Ok(Page::new(
        raw.size.unwrap_or(PageSize::A4),
        raw.orientation.unwrap_or(Orientation::Portrait),
        margins,
        letterhead,
    ))
}

fn margin(value: Option<String>, field: &'static str) -> Result<Option<Length>, TemplateError> {
    value
        .map(|text| Length::parse(&text).map_err(|problem| TemplateError::Value { field, problem }))
        .transpose()
}

fn convert_text(raw: RawText) -> Result<TextStyle, TemplateError> {
    let body = convert_font_style(
        raw.body,
        "text.body.font_families",
        "text.body.size",
        default_font_families(),
        DEFAULT_FONT_SIZE,
    )?;
    let title = convert_font_style(
        raw.title,
        "text.title.font_families",
        "text.title.size",
        default_font_families(),
        DEFAULT_TITLE_SIZE,
    )?;
    let leading = raw
        .leading
        .map(|text| {
            Length::parse(&text).map_err(|problem| TemplateError::Value {
                field: "text.leading",
                problem,
            })
        })
        .transpose()?
        .unwrap_or(DEFAULT_LEADING);
    let paragraph_spacing = raw
        .paragraph_spacing
        .map(|text| {
            Length::parse(&text).map_err(|problem| TemplateError::Value {
                field: "text.paragraph_spacing",
                problem,
            })
        })
        .transpose()?;
    let header_spacing = raw
        .header_spacing
        .map(|text| {
            Length::parse(&text).map_err(|problem| TemplateError::Value {
                field: "text.header_spacing",
                problem,
            })
        })
        .transpose()?;
    let alignment = raw.alignment.unwrap_or(ParagraphAlignment::Justified);
    Ok(TextStyle::new(
        body,
        title,
        leading,
        paragraph_spacing,
        header_spacing,
        alignment,
    ))
}

fn convert_font_style(
    raw: Option<RawFontStyle>,
    families_field: &'static str,
    size_field: &'static str,
    default_families: Vec<FontFamily>,
    default_size: Length,
) -> Result<FontStyle, TemplateError> {
    let Some(raw) = raw else {
        return FontStyle::new(default_families, default_size).map_err(|problem| {
            let field = match problem {
                ValueError::Zero => size_field,
                _ => families_field,
            };
            TemplateError::Value { field, problem }
        });
    };
    let families = match raw.font_families {
        Some(values) if values.is_empty() => {
            return Err(TemplateError::Value {
                field: families_field,
                problem: ValueError::EmptyList,
            });
        }
        Some(values) => values
            .into_iter()
            .map(|value| FontFamily::parse(&value))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|problem| TemplateError::Value {
                field: families_field,
                problem,
            })?,
        None => default_families,
    };
    let size = raw
        .size
        .map(|text| {
            Length::parse(&text).map_err(|problem| TemplateError::Value {
                field: size_field,
                problem,
            })
        })
        .transpose()?
        .unwrap_or(default_size);
    FontStyle::new(families, size).map_err(|problem| {
        let field = match problem {
            ValueError::Zero => size_field,
            _ => families_field,
        };
        TemplateError::Value { field, problem }
    })
}

fn default_font_families() -> Vec<FontFamily> {
    vec![
        FontFamily("Liberation Serif".to_owned()),
        FontFamily("DejaVu Serif".to_owned()),
        FontFamily("Libertinus Serif".to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millimetres_are(points: f64, millimetres: f64) {
        let converted = points / MM_TO_POINTS;
        assert!(
            (converted - millimetres).abs() < 1e-9,
            "{converted} is not {millimetres}"
        );
    }

    #[test]
    fn lengths_parse_the_supported_units() {
        assert_eq!(Length::parse("12pt").map(Length::points), Ok(12.0));
        assert_eq!(Length::parse("12").map(Length::points), Ok(12.0));
        assert_eq!(Length::parse("1in").map(Length::points), Ok(72.0));
        let centimetres = Length::parse("2.54cm").expect("a length");
        millimetres_are(centimetres.points(), 25.4);
        let millimetres = Length::parse("25.4mm").expect("a length");
        millimetres_are(millimetres.points(), 25.4);
    }

    #[test]
    fn lengths_reject_bad_values() {
        assert_eq!(Length::parse("abcmm"), Err(ValueError::InvalidNumber));
        assert_eq!(Length::parse("30px"), Err(ValueError::UnknownUnit));
        assert_eq!(Length::parse("-30mm"), Err(ValueError::Negative));
        assert_eq!(Length::parse("1e999pt"), Err(ValueError::NonFinite));
        assert_eq!(Length::new(f64::NAN), Err(ValueError::NonFinite));
        assert_eq!(Length::new(f64::INFINITY), Err(ValueError::NonFinite));
    }

    #[test]
    fn named_page_sizes_have_the_expected_dimensions() {
        let a4 = PageSize::A4.dimensions();
        millimetres_are(a4.width().points(), 210.0);
        millimetres_are(a4.height().points(), 297.0);

        let letter = PageSize::Letter.dimensions();
        assert_eq!(letter.width().points(), 612.0);
        assert_eq!(letter.height().points(), 792.0);

        let oficio = PageSize::Oficio.dimensions();
        millimetres_are(oficio.width().points(), 216.0);
        millimetres_are(oficio.height().points(), 330.0);
    }

    #[test]
    fn landscape_swaps_the_dimensions() {
        let page = Page::new(
            PageSize::A4,
            Orientation::Landscape,
            Margins::default(),
            None,
        );
        let dimensions = page.dimensions();
        millimetres_are(dimensions.width().points(), 297.0);
        millimetres_are(dimensions.height().points(), 210.0);
    }

    #[test]
    fn font_style_rejects_a_zero_size() {
        let error = FontStyle::new(
            default_font_families(),
            Length::new(0.0).expect("zero is a valid length"),
        );
        assert_eq!(error, Err(ValueError::Zero));
    }

    #[test]
    fn font_style_rejects_an_empty_font_list() {
        let error = FontStyle::new(Vec::new(), DEFAULT_FONT_SIZE);
        assert_eq!(error, Err(ValueError::EmptyList));
    }

    #[test]
    fn title_and_header_defaults_are_distinct_from_the_body() {
        let template = Template::from_yaml("version: 4\n").expect("the default template");
        assert_eq!(template.text().body().size().points(), 12.0);
        assert_eq!(template.text().title().size().points(), 18.0);
        assert_eq!(template.text().leading().points(), 7.8);
        assert_eq!(template.text().header_spacing(), None);
    }

    #[test]
    fn the_embedded_template_is_valid() {
        let template = Template::embedded().expect("the embedded template must be valid");
        assert_eq!(template.schema_version(), SchemaVersion::CURRENT);
        assert_eq!(template.page().size(), PageSize::A4);
        assert_eq!(template.page().orientation(), Orientation::Portrait);
        assert_eq!(template.text().body().font_families().len(), 3);
        assert_eq!(template.text().title().size().points(), 18.0);
    }
}
