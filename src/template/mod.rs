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
pub const CURRENT_SCHEMA_VERSION: SchemaVersion = SchemaVersion { major: 0, minor: 5 };

const DEFAULT_MARGIN: Length = Length(20.0 * MM_TO_POINTS);
const DEFAULT_FONT_SIZE: Length = Length(12.0);
const DEFAULT_LEADING: Length = Length(7.8);
const DEFAULT_TABLE_BORDER: Length = Length(0.5);
const DEFAULT_TABLE_COLOR: Color = Color {
    red: 0xbb,
    green: 0xbb,
    blue: 0xbb,
};
const DEFAULT_TABLE_PADDING: Length = Length(6.0);

/// The tokens a running header or footer slot expands.
const RUNNING_TOKENS: [&str; 4] = ["page", "pages", "title", "author"];

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
    formatting: Formatting,
    table: TableStyle,
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
            formatting: convert_formatting(raw.formatting)?,
            table: convert_table(raw.table)?,
        })
    }

    pub fn schema_version(&self) -> SchemaVersion {
        self.version
    }

    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn formatting(&self) -> &Formatting {
        &self.formatting
    }

    pub fn table(&self) -> &TableStyle {
        &self.table
    }
}

/// The version of the template schema, written as `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchemaVersion {
    major: u16,
    minor: u16,
}

impl SchemaVersion {
    pub const CURRENT: Self = CURRENT_SCHEMA_VERSION;

    pub fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub fn major(self) -> u16 {
        self.major
    }

    pub fn minor(self) -> u16 {
        self.minor
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

impl std::str::FromStr for SchemaVersion {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let (major, minor) = text.split_once('.').ok_or_else(|| {
            format!("`{text}` is not a schema version; use `major.minor`, for example `0.5`")
        })?;
        if major.is_empty() || minor.is_empty() || minor.contains('.') {
            return Err(format!(
                "`{text}` is not a schema version; use `major.minor`, for example `0.5`"
            ));
        }
        let major: u16 = major.parse().map_err(|_| {
            format!("`{text}` is not a schema version; the major part must be a number")
        })?;
        let minor: u16 = minor.parse().map_err(|_| {
            format!("`{text}` is not a schema version; the minor part must be a number")
        })?;
        Ok(Self { major, minor })
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Why a template was rejected.
#[derive(Debug)]
pub enum TemplateError {
    Yaml {
        message: String,
    },
    UnsupportedVersion {
        found: SchemaVersion,
        supported: SchemaVersion,
    },
    Value {
        field: String,
        problem: ValueError,
    },
    UnknownToken {
        field: String,
        token: String,
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
            Self::UnknownToken { field, token } => write!(
                formatter,
                "unknown token `{{{token}}}` in `{field}`; use {{page}}, {{pages}}, {{title}} or {{author}}"
            ),
        }
    }
}

impl std::error::Error for TemplateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yaml { .. }
            | Self::UnsupportedVersion { .. }
            | Self::Value { .. }
            | Self::UnknownToken { .. } => None,
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
    InvalidWeight,
    InvalidColor,
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
            Self::InvalidWeight => "the weight must be a name or a number from 100 to 900",
            Self::InvalidColor => "the color must be written as `#rrggbb`",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ValueError {}

fn value_error(field: impl Into<String>, problem: ValueError) -> TemplateError {
    TemplateError::Value {
        field: field.into(),
        problem,
    }
}

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
    header: Option<RunningText>,
    footer: Option<RunningText>,
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
            header: None,
            footer: None,
        }
    }

    pub fn with_header(self, header: Option<RunningText>) -> Self {
        Self { header, ..self }
    }

    pub fn with_footer(self, footer: Option<RunningText>) -> Self {
        Self { footer, ..self }
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

    pub fn header(&self) -> Option<&RunningText> {
        self.header.as_ref()
    }

    pub fn footer(&self) -> Option<&RunningText> {
        self.footer.as_ref()
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

/// The text of a running header or footer, split into page slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningText {
    slots: Slots,
}

/// The three horizontal slots of a running header or footer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Slots {
    left: Option<String>,
    center: Option<String>,
    right: Option<String>,
}

impl RunningText {
    pub fn new(left: Option<String>, center: Option<String>, right: Option<String>) -> Self {
        let present = |slot: Option<String>| slot.filter(|text| !text.trim().is_empty());
        Self {
            slots: Slots {
                left: present(left),
                center: present(center),
                right: present(right),
            },
        }
    }

    pub fn slots(&self) -> &Slots {
        &self.slots
    }

    pub fn is_empty(&self) -> bool {
        self.slots.left.is_none() && self.slots.center.is_none() && self.slots.right.is_none()
    }
}

impl Slots {
    pub fn left(&self) -> Option<&str> {
        self.left.as_deref()
    }

    pub fn center(&self) -> Option<&str> {
        self.center.as_deref()
    }

    pub fn right(&self) -> Option<&str> {
        self.right.as_deref()
    }
}

/// The table grid style shared by every backend.
#[derive(Debug, Clone, PartialEq)]
pub struct TableStyle {
    border: Length,
    border_color: Color,
    header_fill: Option<Color>,
    cell_padding: Length,
}

impl TableStyle {
    pub fn new(
        border: Length,
        border_color: Color,
        header_fill: Option<Color>,
        cell_padding: Length,
    ) -> Self {
        Self {
            border,
            border_color,
            header_fill,
            cell_padding,
        }
    }

    pub fn border(&self) -> Length {
        self.border
    }

    pub fn border_color(&self) -> Color {
        self.border_color
    }

    pub fn header_fill(&self) -> Option<Color> {
        self.header_fill
    }

    pub fn cell_padding(&self) -> Length {
        self.cell_padding
    }
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            border: DEFAULT_TABLE_BORDER,
            border_color: DEFAULT_TABLE_COLOR,
            header_fill: None,
            cell_padding: DEFAULT_TABLE_PADDING,
        }
    }
}

/// A color written as `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
}

impl Color {
    pub fn parse(text: &str) -> Result<Self, ValueError> {
        let trimmed = text.trim();
        let hex = trimmed.strip_prefix('#').ok_or(ValueError::InvalidColor)?;
        if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
            return Err(ValueError::InvalidColor);
        }
        let pair = |start: usize| -> Result<u8, ValueError> {
            let digits = hex
                .get(start..start.saturating_add(2))
                .ok_or(ValueError::InvalidColor)?;
            u8::from_str_radix(digits, 16).map_err(|_| ValueError::InvalidColor)
        };
        Ok(Self {
            red: pair(0)?,
            green: pair(2)?,
            blue: pair(4)?,
        })
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

/// The text of one running slot.
fn validate_running_text(text: &str, field: &str) -> Result<(), TemplateError> {
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let after = rest.get(start.saturating_add(1)..).unwrap_or_default();
        let Some(end) = after.find('}') else {
            break;
        };
        let token = after.get(..end).unwrap_or_default();
        if is_lowercase_token(token) && !RUNNING_TOKENS.contains(&token) {
            return Err(TemplateError::UnknownToken {
                field: field.to_owned(),
                token: token.to_owned(),
            });
        }
        rest = after.get(end.saturating_add(1)..).unwrap_or_default();
    }
    Ok(())
}

pub(crate) fn is_lowercase_token(token: &str) -> bool {
    let mut characters = token.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// A font weight from 100 to 900.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontWeight(u16);

impl FontWeight {
    pub const NORMAL: Self = Self(400);
    pub const BOLD: Self = Self(700);

    pub fn new(value: u16) -> Result<Self, ValueError> {
        if (100..=900).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ValueError::InvalidWeight)
        }
    }

    pub fn parse(text: &str) -> Result<Self, ValueError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "thin" => Ok(Self(100)),
            "extralight" | "extra-light" => Ok(Self(200)),
            "light" => Ok(Self(300)),
            "normal" | "regular" => Ok(Self(400)),
            "medium" => Ok(Self(500)),
            "semibold" | "semi-bold" | "demibold" => Ok(Self(600)),
            "bold" => Ok(Self(700)),
            "extrabold" | "extra-bold" => Ok(Self(800)),
            "black" | "heavy" => Ok(Self(900)),
            other => other
                .parse::<u16>()
                .map_err(|_| ValueError::InvalidWeight)
                .and_then(Self::new),
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

/// Whether the text is upright or italic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontSlant {
    Normal,
    Italic,
}

impl FontSlant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
        }
    }
}

/// The font families, size, weight and slant for one kind of text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextFace {
    font_families: Vec<FontFamily>,
    size: Length,
    weight: FontWeight,
    slant: FontSlant,
}

impl TextFace {
    pub fn new(
        font_families: Vec<FontFamily>,
        size: Length,
        weight: FontWeight,
        slant: FontSlant,
    ) -> Result<Self, ValueError> {
        if font_families.is_empty() {
            return Err(ValueError::EmptyList);
        }
        if size.points() <= 0.0 {
            return Err(ValueError::Zero);
        }
        Ok(Self {
            font_families,
            size,
            weight,
            slant,
        })
    }

    pub fn font_families(&self) -> &[FontFamily] {
        &self.font_families
    }

    pub fn size(&self) -> Length {
        self.size
    }

    pub fn weight(&self) -> FontWeight {
        self.weight
    }

    pub fn slant(&self) -> FontSlant {
        self.slant
    }
}

/// Paragraph text and the inline styles that override it.
#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphStyle {
    face: TextFace,
    leading: Length,
    spacing: Option<Length>,
    alignment: ParagraphAlignment,
    emphasis: TextFace,
    strong: TextFace,
}

impl ParagraphStyle {
    pub fn new(
        face: TextFace,
        leading: Length,
        spacing: Option<Length>,
        alignment: ParagraphAlignment,
        emphasis: TextFace,
        strong: TextFace,
    ) -> Self {
        Self {
            face,
            leading,
            spacing,
            alignment,
            emphasis,
            strong,
        }
    }

    pub fn face(&self) -> &TextFace {
        &self.face
    }

    pub fn leading(&self) -> Length {
        self.leading
    }

    pub fn spacing(&self) -> Option<Length> {
        self.spacing
    }

    pub fn alignment(&self) -> ParagraphAlignment {
        self.alignment
    }

    pub fn emphasis(&self) -> &TextFace {
        &self.emphasis
    }

    pub fn strong(&self) -> &TextFace {
        &self.strong
    }
}

/// The style of one heading level.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingStyle {
    face: TextFace,
    spacing_before: Length,
    spacing_after: Length,
}

impl HeadingStyle {
    pub fn new(face: TextFace, spacing_before: Length, spacing_after: Length) -> Self {
        Self {
            face,
            spacing_before,
            spacing_after,
        }
    }

    pub fn face(&self) -> &TextFace {
        &self.face
    }

    pub fn spacing_before(&self) -> Length {
        self.spacing_before
    }

    pub fn spacing_after(&self) -> Length {
        self.spacing_after
    }
}

/// The six resolved heading levels.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingStyles {
    levels: [HeadingStyle; 6],
}

impl HeadingStyles {
    pub fn new(levels: [HeadingStyle; 6]) -> Self {
        Self { levels }
    }

    pub fn levels(&self) -> &[HeadingStyle; 6] {
        &self.levels
    }
}

/// The formatting section of a template.
#[derive(Debug, Clone, PartialEq)]
pub struct Formatting {
    paragraph: ParagraphStyle,
    heading: HeadingStyles,
}

impl Formatting {
    pub fn new(paragraph: ParagraphStyle, heading: HeadingStyles) -> Self {
        Self { paragraph, heading }
    }

    pub fn paragraph(&self) -> &ParagraphStyle {
        &self.paragraph
    }

    pub fn heading(&self) -> &HeadingStyles {
        &self.heading
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
    version: SchemaVersion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplate {
    version: SchemaVersion,
    #[serde(default)]
    page: RawPage,
    #[serde(default)]
    formatting: RawFormatting,
    table: Option<RawTable>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    size: Option<PageSize>,
    orientation: Option<Orientation>,
    margins: Option<RawMargins>,
    letterhead: Option<String>,
    header: Option<RawRunning>,
    footer: Option<RawRunning>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunning {
    left: Option<String>,
    center: Option<String>,
    right: Option<String>,
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
struct RawFormatting {
    paragraph: Option<RawParagraph>,
    heading: Option<RawHeading>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawParagraph {
    font_families: Option<Vec<String>>,
    size: Option<String>,
    weight: Option<String>,
    style: Option<FontSlant>,
    leading: Option<String>,
    spacing: Option<String>,
    alignment: Option<ParagraphAlignment>,
    emphasis: Option<RawInlineStyle>,
    strong: Option<RawInlineStyle>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInlineStyle {
    font_families: Option<Vec<String>>,
    size: Option<String>,
    weight: Option<String>,
    style: Option<FontSlant>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeading {
    font_families: Option<Vec<String>>,
    size: Option<String>,
    weight: Option<String>,
    style: Option<FontSlant>,
    spacing_before: Option<String>,
    spacing_after: Option<String>,
    h1: Option<RawHeadingLevel>,
    h2: Option<RawHeadingLevel>,
    h3: Option<RawHeadingLevel>,
    h4: Option<RawHeadingLevel>,
    h5: Option<RawHeadingLevel>,
    h6: Option<RawHeadingLevel>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeadingLevel {
    font_families: Option<Vec<String>>,
    size: Option<String>,
    weight: Option<String>,
    style: Option<FontSlant>,
    spacing_before: Option<String>,
    spacing_after: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DimensionsInput {
    width: String,
    height: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    border: Option<String>,
    border_color: Option<String>,
    header_fill: Option<String>,
    cell_padding: Option<String>,
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
        .map_err(|problem| value_error("page.letterhead", problem))?;
    let header = convert_running(raw.header, "page.header")?;
    let footer = convert_running(raw.footer, "page.footer")?;
    Ok(Page::new(
        raw.size.unwrap_or(PageSize::A4),
        raw.orientation.unwrap_or(Orientation::Portrait),
        margins,
        letterhead,
    )
    .with_header(header)
    .with_footer(footer))
}

fn convert_running(
    raw: Option<RawRunning>,
    prefix: &str,
) -> Result<Option<RunningText>, TemplateError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    for (slot, value) in [
        ("left", &raw.left),
        ("center", &raw.center),
        ("right", &raw.right),
    ] {
        if let Some(text) = value {
            validate_running_text(text, &format!("{prefix}.{slot}"))?;
        }
    }
    let running = RunningText::new(raw.left, raw.center, raw.right);
    Ok(if running.is_empty() {
        None
    } else {
        Some(running)
    })
}

fn convert_table(raw: Option<RawTable>) -> Result<TableStyle, TemplateError> {
    let defaults = TableStyle::default();
    let Some(raw) = raw else {
        return Ok(defaults);
    };
    let border = parse_size_field(raw.border, "table.border", defaults.border())?;
    let border_color = raw
        .border_color
        .map(|text| Color::parse(&text))
        .transpose()
        .map_err(|problem| value_error("table.border_color", problem))?
        .unwrap_or(defaults.border_color());
    let header_fill = raw
        .header_fill
        .map(|text| Color::parse(&text))
        .transpose()
        .map_err(|problem| value_error("table.header_fill", problem))?;
    let cell_padding = parse_size_field(
        raw.cell_padding,
        "table.cell_padding",
        defaults.cell_padding(),
    )?;
    Ok(TableStyle::new(
        border,
        border_color,
        header_fill,
        cell_padding,
    ))
}

fn margin(value: Option<String>, field: &'static str) -> Result<Option<Length>, TemplateError> {
    value
        .map(|text| parse_length_field(&text, field))
        .transpose()
}

fn convert_formatting(raw: RawFormatting) -> Result<Formatting, TemplateError> {
    Ok(Formatting::new(
        convert_paragraph(raw.paragraph.unwrap_or_default())?,
        convert_headings(raw.heading.unwrap_or_default())?,
    ))
}

fn convert_paragraph(raw: RawParagraph) -> Result<ParagraphStyle, TemplateError> {
    let families = parse_families_field(
        raw.font_families,
        "formatting.paragraph.font_families",
        default_paragraph_font_families(),
    )?;
    let size = parse_size_field(raw.size, "formatting.paragraph.size", DEFAULT_FONT_SIZE)?;
    if size.points() <= 0.0 {
        return Err(value_error("formatting.paragraph.size", ValueError::Zero));
    }
    let weight = parse_weight_field(
        raw.weight,
        "formatting.paragraph.weight",
        FontWeight::NORMAL,
    )?;
    let slant = raw.style.unwrap_or(FontSlant::Normal);
    let face = TextFace::new(families, size, weight, slant)
        .map_err(|problem| value_error("formatting.paragraph", problem))?;
    let leading = parse_size_field(raw.leading, "formatting.paragraph.leading", DEFAULT_LEADING)?;
    let spacing = parse_optional_length(raw.spacing, "formatting.paragraph.spacing")?;
    let alignment = raw.alignment.unwrap_or(ParagraphAlignment::Justified);
    let emphasis = convert_inline(
        &face,
        raw.emphasis,
        "formatting.paragraph.emphasis",
        Some(FontSlant::Italic),
        None,
    )?;
    let strong = convert_inline(
        &face,
        raw.strong,
        "formatting.paragraph.strong",
        None,
        Some(FontWeight::BOLD),
    )?;
    Ok(ParagraphStyle::new(
        face, leading, spacing, alignment, emphasis, strong,
    ))
}

fn convert_inline(
    base: &TextFace,
    raw: Option<RawInlineStyle>,
    prefix: &str,
    semantic_slant: Option<FontSlant>,
    semantic_weight: Option<FontWeight>,
) -> Result<TextFace, TemplateError> {
    let mut families = base.font_families().to_vec();
    let mut size = base.size();
    let mut weight = semantic_weight.unwrap_or(base.weight());
    let mut slant = semantic_slant.unwrap_or(base.slant());
    if let Some(raw) = raw {
        if let Some(values) = raw.font_families {
            families = parse_families(values, &format!("{prefix}.font_families"))?;
        }
        if let Some(text) = raw.size {
            size = parse_length_field(&text, &format!("{prefix}.size"))?;
        }
        if let Some(text) = raw.weight {
            weight = parse_weight(&text, &format!("{prefix}.weight"))?;
        }
        if let Some(value) = raw.style {
            slant = value;
        }
    }
    if size.points() <= 0.0 {
        return Err(value_error(format!("{prefix}.size"), ValueError::Zero));
    }
    TextFace::new(families, size, weight, slant).map_err(|problem| value_error(prefix, problem))
}

fn convert_headings(raw: RawHeading) -> Result<HeadingStyles, TemplateError> {
    let RawHeading {
        font_families,
        size,
        weight,
        style,
        spacing_before,
        spacing_after,
        h1,
        h2,
        h3,
        h4,
        h5,
        h6,
    } = raw;
    let base = HeadingOverrides::parse(
        font_families,
        size,
        weight,
        style,
        spacing_before,
        spacing_after,
        "formatting.heading",
    )?;
    let raw_levels = [h1, h2, h3, h4, h5, h6];
    let mut overrides: [Option<HeadingOverrides>; 6] = Default::default();
    for (index, raw_level) in raw_levels.into_iter().enumerate() {
        if let Some(raw_level) = raw_level {
            let prefix = format!("formatting.heading.h{}", index + 1);
            overrides[index] = Some(HeadingOverrides::parse(
                raw_level.font_families,
                raw_level.size,
                raw_level.weight,
                raw_level.style,
                raw_level.spacing_before,
                raw_level.spacing_after,
                &prefix,
            )?);
        }
    }
    let levels = std::array::from_fn(|index| {
        let mut style = default_heading_style(index + 1);
        base.apply(&mut style);
        if let Some(overrides) = &overrides[index] {
            overrides.apply(&mut style);
        }
        style
    });
    Ok(HeadingStyles::new(levels))
}

#[derive(Debug, Default)]
struct HeadingOverrides {
    font_families: Option<Vec<FontFamily>>,
    size: Option<Length>,
    weight: Option<FontWeight>,
    slant: Option<FontSlant>,
    spacing_before: Option<Length>,
    spacing_after: Option<Length>,
}

impl HeadingOverrides {
    fn parse(
        font_families: Option<Vec<String>>,
        size: Option<String>,
        weight: Option<String>,
        slant: Option<FontSlant>,
        spacing_before: Option<String>,
        spacing_after: Option<String>,
        prefix: &str,
    ) -> Result<Self, TemplateError> {
        let size = size
            .map(|text| parse_length_field(&text, &format!("{prefix}.size")))
            .transpose()?;
        if let Some(size) = size
            && size.points() <= 0.0
        {
            return Err(value_error(format!("{prefix}.size"), ValueError::Zero));
        }
        Ok(Self {
            font_families: font_families
                .map(|values| parse_families(values, &format!("{prefix}.font_families")))
                .transpose()?,
            size,
            weight: weight
                .map(|text| parse_weight(&text, &format!("{prefix}.weight")))
                .transpose()?,
            slant,
            spacing_before: spacing_before
                .map(|text| parse_length_field(&text, &format!("{prefix}.spacing_before")))
                .transpose()?,
            spacing_after: spacing_after
                .map(|text| parse_length_field(&text, &format!("{prefix}.spacing_after")))
                .transpose()?,
        })
    }

    fn apply(&self, style: &mut HeadingStyle) {
        if let Some(families) = &self.font_families {
            style.face.font_families = families.clone();
        }
        if let Some(size) = self.size {
            style.face.size = size;
        }
        if let Some(weight) = self.weight {
            style.face.weight = weight;
        }
        if let Some(slant) = self.slant {
            style.face.slant = slant;
        }
        if let Some(before) = self.spacing_before {
            style.spacing_before = before;
        }
        if let Some(after) = self.spacing_after {
            style.spacing_after = after;
        }
    }
}

fn parse_families(values: Vec<String>, field: &str) -> Result<Vec<FontFamily>, TemplateError> {
    if values.is_empty() {
        return Err(value_error(field, ValueError::EmptyList));
    }
    values
        .into_iter()
        .map(|value| FontFamily::parse(&value).map_err(|problem| value_error(field, problem)))
        .collect()
}

fn parse_families_field(
    value: Option<Vec<String>>,
    field: &str,
    default: Vec<FontFamily>,
) -> Result<Vec<FontFamily>, TemplateError> {
    match value {
        Some(values) => parse_families(values, field),
        None => Ok(default),
    }
}

fn parse_length_field(text: &str, field: &str) -> Result<Length, TemplateError> {
    Length::parse(text).map_err(|problem| value_error(field, problem))
}

fn parse_size_field(
    value: Option<String>,
    field: &str,
    default: Length,
) -> Result<Length, TemplateError> {
    match value {
        Some(text) => parse_length_field(&text, field),
        None => Ok(default),
    }
}

fn parse_weight(text: &str, field: &str) -> Result<FontWeight, TemplateError> {
    FontWeight::parse(text).map_err(|problem| value_error(field, problem))
}

fn parse_weight_field(
    value: Option<String>,
    field: &str,
    default: FontWeight,
) -> Result<FontWeight, TemplateError> {
    match value {
        Some(text) => parse_weight(&text, field),
        None => Ok(default),
    }
}

fn parse_optional_length(
    value: Option<String>,
    field: &str,
) -> Result<Option<Length>, TemplateError> {
    value
        .map(|text| parse_length_field(&text, field))
        .transpose()
}

fn default_paragraph_font_families() -> Vec<FontFamily> {
    vec![
        FontFamily("Liberation Serif".to_owned()),
        FontFamily("DejaVu Serif".to_owned()),
        FontFamily("Libertinus Serif".to_owned()),
    ]
}

fn default_heading_font_families() -> Vec<FontFamily> {
    vec![
        FontFamily("Liberation Sans".to_owned()),
        FontFamily("DejaVu Sans".to_owned()),
        FontFamily("Libertinus Serif".to_owned()),
    ]
}

const HEADING_SIZES: [f64; 6] = [18.0, 14.0, 12.0, 12.0, 12.0, 12.0];
const HEADING_SPACING_BEFORE: [f64; 6] = [18.0, 12.0, 10.0, 8.0, 8.0, 8.0];
const HEADING_SPACING_AFTER: [f64; 6] = [6.0, 4.0, 4.0, 2.0, 2.0, 2.0];

fn default_heading_style(level: usize) -> HeadingStyle {
    let index = level.saturating_sub(1).min(5);
    HeadingStyle::new(
        TextFace {
            font_families: default_heading_font_families(),
            size: Length(HEADING_SIZES[index]),
            weight: FontWeight::BOLD,
            slant: FontSlant::Normal,
        },
        Length(HEADING_SPACING_BEFORE[index]),
        Length(HEADING_SPACING_AFTER[index]),
    )
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
    fn text_face_rejects_a_zero_size() {
        let error = TextFace::new(
            default_paragraph_font_families(),
            Length::new(0.0).expect("zero is a valid length"),
            FontWeight::NORMAL,
            FontSlant::Normal,
        );
        assert_eq!(error, Err(ValueError::Zero));
    }

    #[test]
    fn text_face_rejects_an_empty_font_list() {
        let error = TextFace::new(
            Vec::new(),
            DEFAULT_FONT_SIZE,
            FontWeight::NORMAL,
            FontSlant::Normal,
        );
        assert_eq!(error, Err(ValueError::EmptyList));
    }

    #[test]
    fn font_weight_parses_names_and_numbers() {
        assert_eq!(FontWeight::parse("normal"), Ok(FontWeight::NORMAL));
        assert_eq!(FontWeight::parse("semibold").map(FontWeight::get), Ok(600));
        assert_eq!(FontWeight::parse("700").map(FontWeight::get), Ok(700));
        assert_eq!(FontWeight::parse("50"), Err(ValueError::InvalidWeight));
        assert_eq!(
            FontWeight::parse("heavyweight"),
            Err(ValueError::InvalidWeight)
        );
    }

    #[test]
    fn paragraph_and_heading_defaults_are_distinct() {
        let template = Template::from_yaml("version: \"0.5\"\n").expect("the default template");
        let paragraph = template.formatting().paragraph();
        assert_eq!(paragraph.face().size().points(), 12.0);
        assert_eq!(paragraph.face().weight(), FontWeight::NORMAL);
        assert_eq!(paragraph.face().slant(), FontSlant::Normal);
        assert_eq!(paragraph.leading().points(), 7.8);
        assert_eq!(paragraph.emphasis().slant(), FontSlant::Italic);
        assert_eq!(paragraph.strong().weight(), FontWeight::BOLD);

        let heading = template.formatting().heading().levels();
        assert_eq!(heading[0].face().size().points(), 18.0);
        assert_eq!(heading[1].face().size().points(), 14.0);
        assert_eq!(heading[2].face().size().points(), 12.0);
        assert_eq!(heading[0].face().weight(), FontWeight::BOLD);
    }

    #[test]
    fn schema_versions_parse_major_minor() {
        assert_eq!(
            "0.5"
                .parse::<SchemaVersion>()
                .map(|version| version.to_string()),
            Ok("0.5".to_owned())
        );
        assert!("0.5.0".parse::<SchemaVersion>().is_err());
        assert!("4".parse::<SchemaVersion>().is_err());
    }

    #[test]
    fn the_embedded_template_is_valid() {
        let template = Template::embedded().expect("the embedded template must be valid");
        assert_eq!(template.schema_version(), SchemaVersion::CURRENT);
        assert_eq!(template.page().size(), PageSize::A4);
        assert_eq!(template.page().orientation(), Orientation::Portrait);
        assert_eq!(
            template
                .formatting()
                .paragraph()
                .face()
                .font_families()
                .len(),
            3
        );
        assert_eq!(
            template.formatting().heading().levels()[0]
                .face()
                .size()
                .points(),
            18.0
        );
    }
}
