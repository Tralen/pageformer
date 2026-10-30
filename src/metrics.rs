//! Shared layout metrics for the office backends.
//!
//! Typst advances a line by the template leading plus the font's cap height,
//! which is about 0.657 of the size. The office formats reproduce that model so
//! their spacing stays close to the PDF.

use crate::template::Length;

/// The ratio of a font's cap height to its size.
pub(crate) const CAP_HEIGHT_RATIO: f64 = 0.657;
/// How much of the heading space above the line box the editor already adds.
pub(crate) const HEADING_TOP_TRIM: f64 = 6.0;
/// How much of the heading space below the line box the editor already adds.
pub(crate) const HEADING_BOTTOM_TRIM: f64 = 10.0;

/// The line advance Typst produces: the leading plus the cap height.
pub(crate) fn line_advance(size: Length, leading: Length) -> f64 {
    leading.points() + size.points() * CAP_HEIGHT_RATIO
}

/// The space between paragraphs: the template spacing minus the leading.
pub(crate) fn paragraph_after(spacing: Option<Length>, leading: Length) -> f64 {
    match spacing {
        Some(spacing) => (spacing.points() - leading.points()).max(0.0),
        None => 0.0,
    }
}

/// The space above a heading, discounted by the paragraph margin and the line
/// box the editor adds.
pub(crate) fn heading_before(
    spacing: Length,
    leading: Length,
    paragraph_spacing: Option<Length>,
) -> f64 {
    (spacing.points() - paragraph_after(paragraph_spacing, leading) - HEADING_TOP_TRIM).max(0.0)
}

/// The space below a heading, discounted by the line box the editor adds.
pub(crate) fn heading_after(spacing: Length) -> f64 {
    (spacing.points() - HEADING_BOTTOM_TRIM).max(0.0)
}

/// A point value in twentieths of a point, for OOXML.
pub(crate) fn twips(points: f64) -> String {
    format!("{:.0}", (points * 20.0).round())
}

/// A point value in half-points, for `w:sz`.
pub(crate) fn half_points(points: f64) -> String {
    format!("{:.0}", (points * 2.0).round())
}

/// A point value in eighths of a point, for border widths.
///
/// OOXML accepts widths from 2 (a quarter point) to 96 eighths, so thinner or
/// thicker borders are clamped to that range.
pub(crate) fn eighths(points: f64) -> String {
    format!("{:.0}", (points * 8.0).round().clamp(2.0, 96.0))
}

/// A point value for the gap between a border and its paragraph, for `w:space`.
///
/// OOXML accepts gaps from 0 to 31 points.
pub(crate) fn border_space(points: f64) -> String {
    format!("{:.0}", points.round().clamp(0.0, 31.0))
}

/// A point value in English Metric Units, for DrawingML.
pub(crate) fn emu(points: f64) -> String {
    format!("{:.0}", (points * 12_700.0).round())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn length(points: f64) -> Length {
        Length::new(points).expect("a valid length")
    }

    #[test]
    fn the_line_advance_adds_the_leading_to_the_cap_height() {
        assert!((line_advance(length(12.0), length(8.0)) - 15.884).abs() < 0.001);
    }

    #[test]
    fn the_paragraph_after_never_goes_negative() {
        assert_eq!(paragraph_after(Some(length(24.0)), length(8.0)), 16.0);
        assert_eq!(paragraph_after(Some(length(4.0)), length(8.0)), 0.0);
        assert_eq!(paragraph_after(None, length(8.0)), 0.0);
    }

    #[test]
    fn ooxml_units_round_to_whole_numbers() {
        assert_eq!(twips(15.9), "318");
        assert_eq!(half_points(12.0), "24");
        assert_eq!(eighths(0.5), "4");
        assert_eq!(eighths(0.1), "2");
        assert_eq!(eighths(30.0), "96");
        assert_eq!(border_space(3.0), "3");
        assert_eq!(border_space(40.0), "31");
        assert_eq!(emu(72.0), "914400");
    }
}
