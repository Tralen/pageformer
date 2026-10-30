//! XML helpers shared by the ODF parts.

use std::fmt::Write as _;

use crate::template::Length;

/// Escapes text content.
pub fn escape_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    push_escaped(&mut output, text, false);
    output
}

/// Escapes an attribute value.
pub fn escape_attribute(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    push_escaped(&mut output, text, true);
    output
}

fn push_escaped(output: &mut String, text: &str, attribute: bool) {
    for character in text.chars() {
        if !is_xml_character(character) {
            output.push('\u{fffd}');
            continue;
        }
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if attribute => output.push_str("&quot;"),
            '\'' if attribute => output.push_str("&apos;"),
            other => output.push(other),
        }
    }
}

fn is_xml_character(character: char) -> bool {
    matches!(character,
        '\u{9}' | '\u{a}' | '\u{d}'
        | '\u{20}'..='\u{d7ff}'
        | '\u{e000}'..='\u{fffd}'
        | '\u{10000}'..='\u{10ffff}')
}

/// Formats a length in points, such as `12.00pt`.
pub fn points(length: Length) -> String {
    format!("{:.2}pt", length.points())
}

/// Formats a number of points, such as `12.00pt`.
pub fn points_value(points: f64) -> String {
    format!("{:.2}pt", points)
}

/// Formats a length in centimetres, such as `21.000cm`.
pub fn centimetres(length: Length) -> String {
    centimetres_points(length.points())
}

/// Formats a length given in points as centimetres, such as `21.000cm`.
pub fn centimetres_points(points: f64) -> String {
    format!("{:.3}cm", points / 72.0 * 2.54)
}

/// Whether a value is exactly zero points.
pub fn is_zero(length: Length) -> bool {
    length.points() == 0.0
}

/// Writes a string without failing, because writing to a `String` cannot fail.
pub fn push(output: &mut String, text: &str) {
    let _ = output.write_str(text);
}

/// Writes formatted text without failing, because writing to a `String` cannot fail.
pub fn push_fmt(output: &mut String, arguments: std::fmt::Arguments<'_>) {
    let _ = output.write_fmt(arguments);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::Length;

    #[test]
    fn text_and_attribute_escaping_differ_only_in_quotes() {
        assert_eq!(
            escape_text("a & b < c > d \"e\""),
            "a &amp; b &lt; c &gt; d \"e\""
        );
        assert_eq!(
            escape_attribute("a & b < c > d \"e\""),
            "a &amp; b &lt; c &gt; d &quot;e&quot;"
        );
    }

    #[test]
    fn a_control_character_becomes_the_replacement_character() {
        assert_eq!(escape_text("a\u{1}b"), "a\u{fffd}b");
    }

    #[test]
    fn lengths_use_the_expected_units() {
        let length = Length::parse("72pt").expect("a valid length");

        assert_eq!(points(length), "72.00pt");
        assert_eq!(centimetres(length), "2.540cm");
    }
}
