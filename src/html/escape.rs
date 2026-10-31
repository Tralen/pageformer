//! HTML escaping.
//!
//! HTML and XML share the characters that must be escaped in text and
//! attributes, so the shared implementation stays in [`crate::xml`]. Keeping the
//! wrapper here makes the HTML boundary explicit and leaves room for a
//! backend-specific rule later.

/// Escapes text content.
pub(crate) fn text(value: &str) -> String {
    crate::xml::escape_text(value)
}

/// Escapes an attribute value.
pub(crate) fn attribute(value: &str) -> String {
    crate::xml::escape_attribute(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_attribute_escaping_follow_the_xml_rules() {
        assert_eq!(text("a & b < c > d"), "a &amp; b &lt; c &gt; d");
        assert_eq!(attribute("a \"b\" 'c'"), "a &quot;b&quot; &apos;c&apos;");
        assert_eq!(text("a\u{1}b"), "a\u{fffd}b");
    }
}
