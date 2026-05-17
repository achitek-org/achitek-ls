//! User-facing API for analyzing Tera source.
//!
//! The API is forgiving: [`AnalysisError`] is reserved for infrastructure
//! failures such as parser setup failures. Tera source violations are intended
//! to be returned as structured diagnostics on [`Analysis`] as the diagnostics
//! pass grows.

mod diagnostics;
mod lowering;
mod syntax;

use crate::{
    Diagnostic,
    model::TeraFile,
    parser::{self, ParseError},
};
use std::{
    backtrace::Backtrace,
    error::Error as StdError,
    fmt::{Display, Formatter},
};

/// A forgiving analysis result for Tera source.
#[derive(Debug, Clone)]
pub struct Analysis<'a> {
    source: &'a str,
    file: TeraFile,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Analysis<'a> {
    /// Returns the source text analyzed by this result.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// Returns the recovered Tera semantic model.
    pub fn file(&self) -> &TeraFile {
        &self.file
    }

    /// Returns diagnostics discovered while analyzing the source.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns true when any diagnostic has error severity.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity() == crate::Severity::Error)
    }
}

/// Errors that prevent Tera analysis from running.
///
/// Normal source violations are returned as diagnostics in [`Analysis`], not as
/// [`AnalysisError`].
#[derive(Debug)]
pub struct AnalysisError {
    kind: AnalysisErrorKind,
    backtrace: Backtrace,
}

impl AnalysisError {
    /// Returns true when analysis failed before parsing completed.
    pub fn is_parse(&self) -> bool {
        matches!(self.kind, AnalysisErrorKind::Parse(_))
    }

    /// Returns the underlying parse error, if parsing failed.
    pub fn parse_error(&self) -> Option<&ParseError> {
        match &self.kind {
            AnalysisErrorKind::Parse(source) => Some(source),
        }
    }

    /// Returns the backtrace captured when the error was created.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

impl From<ParseError> for AnalysisError {
    fn from(source: ParseError) -> Self {
        Self {
            kind: AnalysisErrorKind::Parse(source),
            backtrace: Backtrace::capture(),
        }
    }
}

impl Display for AnalysisError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            AnalysisErrorKind::Parse(source) => {
                writeln!(f, "failed to parse Tera source: {source}")?;
            }
        }

        write!(f, "backtrace:\n{}", self.backtrace)
    }
}

impl StdError for AnalysisError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.kind {
            AnalysisErrorKind::Parse(source) => Some(source),
        }
    }
}

#[derive(Debug)]
enum AnalysisErrorKind {
    Parse(ParseError),
}

/// Analyzes Tera source and returns a forgiving semantic result.
///
/// This function only returns an error when the parser cannot be configured or
/// Tree-sitter does not produce a parse tree. Invalid Tera source is intended
/// to be reported through [`Analysis::diagnostics`] instead.
///
/// # Errors
///
/// Returns [`AnalysisError`] if low-level Tree-sitter parsing cannot be started
/// or does not produce a parse tree.
pub fn analyze(source: &str) -> Result<Analysis<'_>, AnalysisError> {
    let tree = parser::parse(source)?;
    let file = TeraFile::from_tree(&tree, source);
    let diagnostics = diagnostics::collect_diagnostics(&tree, source);

    Ok(Analysis {
        source,
        file,
        diagnostics,
    })
}
