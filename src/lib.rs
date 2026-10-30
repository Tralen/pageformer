//! Pageformer core: Markdown parsing, the document model, the template and the
//! shared export pipeline.
//!
//! The pure pieces are the parser, the model, the template and the renderers.
//! The `pipeline` module is the explicitly named effectful edge: it reads the
//! source, resolves the template and the confidential fields, renders and writes
//! the output. File discovery stays in the binary. Every expected failure is a
//! value instead of a panic.

#![forbid(unsafe_code)]

pub mod assets;
pub mod confidential;
pub mod document;
pub mod docx;
pub mod format;
pub mod markdown;
pub mod odt;
pub mod pdf;
pub mod pipeline;
pub mod render;
pub mod template;
pub mod txt;

mod image;
mod metrics;
mod xml;
mod zip;
