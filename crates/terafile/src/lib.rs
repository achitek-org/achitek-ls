//! Tree-sitter backed semantic parser for Tera template source.
//!
//! `terafile` provides a forgiving analysis API for editor and tooling
//! workflows. Invalid or incomplete Tera source is reported as structured
//! diagnostics whenever Tree-sitter can still recover a syntax tree.
//!
//! # Example
//!
//! ```
//! let analysis = terafile::analyze(r#"Hello {{ user.name | upper }}"#)?;
//!
//! assert!(!analysis.has_errors());
//! assert_eq!(analysis.file().filters()[0].value.name, "upper");
//! assert_eq!(
//!     analysis.file().variable_references()[0].value.path,
//!     "user.name"
//! );
//! # Ok::<(), terafile::AnalysisError>(())
//! ```

#![deny(missing_docs)]

mod analysis;
mod diagnostics;
mod model;
mod parser;
mod tree_sitter_tera;

pub use achitek_source::Spanned;
pub use analysis::{Analysis, AnalysisError, analyze};
pub use diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticKind, Severity, TextPosition, TextRange,
};
pub use model::{
    Binding, BindingKind, Macro, MacroCall, MacroParameter, NamedReference, TemplateDependency,
    TemplateDependencyKind, TemplatePath, TeraFile, VariableReference,
};
pub use parser::{ParseError, parse};
