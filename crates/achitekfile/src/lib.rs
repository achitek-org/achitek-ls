//! A [Tree-sitter] backed semantic parser for Achitekfile source.
//!
//! `achitekfile` wraps the [tree-sitter-achitekfile] grammar and exposes a
//! small semantic API over the concrete Tree-sitter syntax tree. Start with
//! [`analyze`] for semantic analysis or [`parse_tree`] if you need direct
//! Tree-sitter access.
//!
//! # Examples
//!
//! ```
//! let source = r#"
//! blueprint {
//!   version = "1.0.0"
//!   name = "web-app"
//! }
//!
//! prompt "database" {
//!   type = select
//!   choices = ["postgres", "sqlite"]
//!   default = "postgres"
//! }
//!
//! prompt "orm" {
//!   type = select
//!   choices = ["sqlx", "diesel"]
//!   depends_on = database != "sqlite"
//! }
//! "#;
//!
//! let file = achitekfile::analyze(source)?.into_valid().map_err(|diagnostics| {
//!     let message = diagnostics
//!         .into_iter()
//!         .map(|diagnostic| diagnostic.message().to_owned())
//!         .collect::<Vec<_>>()
//!         .join(", ");
//!     std::io::Error::new(std::io::ErrorKind::InvalidData, message)
//! })?;
//!
//! assert_eq!(file.blueprint().name, "web-app");
//! assert_eq!(file.prompts().len(), 2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! [Tree-sitter]: https://tree-sitter.github.io/tree-sitter/
//! [tree-sitter-achitekfile]: https://docs.rs/tree-sitter-achitekfile/0.1.0/tree_sitter_achitekfile/

#![deny(missing_docs)]

mod analysis;
mod diagnostics;
pub mod model;
mod parser;
mod sort;

#[doc(inline)]
pub use analysis::{Analysis, AnalysisError, analyze};
#[doc(inline)]
pub use diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticKind, Severity, TextPosition, TextRange,
};
#[doc(inline)]
pub use parser::{ParseError, parse_tree};
#[doc(inline)]
pub use sort::{Cycle, SortError};
