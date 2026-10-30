//! Pageformer core: Markdown parsing, the document model and the template.
//!
//! The library is the pure core of the application. File discovery, reading and
//! writing live in the binary or in the explicitly named loader functions, and
//! every expected failure is a value instead of a panic.

#![forbid(unsafe_code)]

pub mod confidential;
pub mod document;
pub mod markdown;
pub mod pdf;
pub mod render;
pub mod template;
pub mod txt;
