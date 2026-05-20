//! Low-level Tree-sitter parsing for Tera template source.

use std::backtrace::Backtrace;
use tree_sitter::{Language, Parser, Tree};

/// Parses Tera source text into a Tree-sitter syntax tree.
///
/// This is a low-level API. Prefer [`crate::analyze`] unless you specifically
/// need direct Tree-sitter access.
///
/// ```
/// let tree = terafile::parse(r#"Hello {{ name }}"#)?;
///
/// assert_eq!(tree.root_node().kind(), "source_file");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`ParseError`] if the parser cannot be configured with the Tera
/// grammar or if Tree-sitter does not produce a tree.
pub fn parse(source: &str) -> Result<Tree, ParseError> {
    let mut parser = Parser::new();
    let language: Language = crate::tree_sitter_tera::LANGUAGE.into();
    parser.set_language(&language)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(ParseError::parse_cancelled)?;

    Ok(tree)
}

/// Errors that can occur while parsing Tera source.
///
/// See [`parse`] for an example of handling parser setup and Tree-sitter parse
/// failures with `?`.
#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
    backtrace: Backtrace,
}

impl ParseError {
    /// Returns true when parser setup failed because the Tree-sitter language
    /// could not be configured.
    ///
    /// See [`parse`] for a complete example.
    pub fn is_language(&self) -> bool {
        matches!(self.kind, ParseErrorKind::Language(_))
    }

    /// Returns true when Tree-sitter did not produce a parse tree.
    ///
    /// See [`parse`] for a complete example.
    pub fn is_parse_cancelled(&self) -> bool {
        matches!(self.kind, ParseErrorKind::ParseCancelled)
    }

    /// Returns the underlying Tree-sitter language error, if parser setup
    /// failed.
    ///
    /// See [`parse`] for a complete example.
    pub fn language_error(&self) -> Option<&tree_sitter::LanguageError> {
        match &self.kind {
            ParseErrorKind::Language(source) => Some(source),
            ParseErrorKind::ParseCancelled => None,
        }
    }

    /// Returns the backtrace captured when the error was created.
    ///
    /// See [`parse`] for a complete example.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    fn language(source: tree_sitter::LanguageError) -> Self {
        Self {
            kind: ParseErrorKind::Language(source),
            backtrace: Backtrace::capture(),
        }
    }

    fn parse_cancelled() -> Self {
        Self {
            kind: ParseErrorKind::ParseCancelled,
            backtrace: Backtrace::capture(),
        }
    }
}

#[derive(Debug)]
enum ParseErrorKind {
    Language(tree_sitter::LanguageError),
    ParseCancelled,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ParseErrorKind::Language(error) => {
                writeln!(f, "failed to configure the Tera parser: {error}")?;
            }
            ParseErrorKind::ParseCancelled => {
                writeln!(f, "tree-sitter did not produce a parse tree")?;
            }
        }

        write!(f, "backtrace:\n{}", self.backtrace)
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ParseErrorKind::Language(error) => Some(error),
            ParseErrorKind::ParseCancelled => None,
        }
    }
}

impl From<tree_sitter::LanguageError> for ParseError {
    fn from(error: tree_sitter::LanguageError) -> Self {
        Self::language(error)
    }
}
