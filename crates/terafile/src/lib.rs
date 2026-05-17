//! Tree sitter backed semantic parser for .tera files

#![deny(missing_docs)]

mod analysis;
mod diagnostics;
mod model;
mod parser;
mod tree_sitter_tera;

pub use analysis::{Analysis, AnalysisError, analyze};
pub use diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticKind, Severity, TextPosition, TextRange,
};
pub use model::{
    Binding, BindingKind, Macro, MacroCall, MacroParameter, NamedReference, Spanned,
    TemplateDependency, TemplateDependencyKind, TemplatePath, TeraFile, VariableReference,
};
pub use parser::{ParseError, parse};
