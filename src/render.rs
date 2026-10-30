//! The renderer boundary.
//!
//! The pipeline parses the Markdown, resolves the template and the confidential
//! fields, prepares the assets a backend needs and then calls one backend. A
//! backend receives the resolved document and the validated template and
//! returns bytes plus warnings; it never parses Markdown, reads a file, reads
//! the environment, reads the clock or writes to disk.

use crate::document::Document;
use crate::template::Template;

/// The bytes a backend produced and the warnings it reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    bytes: Vec<u8>,
    warnings: Vec<String>,
}

impl Rendered {
    pub fn new(bytes: Vec<u8>, warnings: Vec<String>) -> Self {
        Self { bytes, warnings }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

/// The contract every output format implements.
///
/// The pipeline prepares everything a backend needs before [`render`] is
/// called, so the method is pure. A backend that cannot fail uses
/// [`std::convert::Infallible`] as its error type.
///
/// [`render`]: Renderer::render
pub trait Renderer {
    /// Why this backend can fail.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Renders a resolved document with a validated template.
    fn render(&self, document: &Document, template: &Template) -> Result<Rendered, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rendered_value_keeps_its_bytes_and_warnings() {
        let rendered = Rendered::new(b"texto".to_vec(), vec!["aviso".to_owned()]);

        assert_eq!(rendered.bytes(), b"texto");
        assert_eq!(rendered.warnings(), ["aviso"]);
        assert_eq!(rendered.into_bytes(), b"texto");
    }
}
