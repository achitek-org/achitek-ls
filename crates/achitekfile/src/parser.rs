use std::{
    backtrace::Backtrace,
    error::Error as StdError,
    fmt::{Display, Formatter},
};
use tree_sitter::{Language, Parser, Tree};

/// Parses Achitekfile source text into a Tree-sitter [`Tree`].
///
/// This is a low level API that you generally shouldn't need to use.
///
/// This function configures a Tree-sitter parser with the Achitekfile grammar,
/// parses the supplied source, and returns the raw Tree-sitter parse tree.
///
/// Prefer [`crate::analyze`] unless you specifically need low-level
/// Tree-sitter access.
///
/// ```
/// let source = r#"
/// blueprint {
///   version = "1.0.0"
///   name = "example"
/// }
///
/// prompt "project_name" {
///   type = string
///   help = "Project name"
/// }
/// "#;
///
/// let tree = achitekfile::parse_tree(source)?;
///
/// assert_eq!(tree.root_node().kind(), "file");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`ParseError`] if the parser cannot be configured with the Achitek
/// grammar or if Tree-sitter does not produce a tree.
pub fn parse_tree(source: &str) -> Result<Tree, ParseError> {
    let mut parser = Parser::new();
    let language: Language = tree_sitter_achitekfile::LANGUAGE.into();
    parser.set_language(&language)?;
    let ast: Tree = parser
        .parse(source, None)
        .ok_or_else(ParseError::parse_cancelled)?;

    Ok(ast)
}

/// Errors that can occur while parsing source text into a Tree-sitter [`Tree`].
///
/// See [`parse_tree`] for an example of handling parser setup and Tree-sitter
/// parse failures with `?`.
#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
    backtrace: Backtrace,
}

impl ParseError {
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

    /// Returns true when parsing was cancelled before producing a tree.
    ///
    /// See [`parse_tree`] for a complete example.
    pub fn is_parse_cancelled(&self) -> bool {
        matches!(self.kind, ParseErrorKind::ParseCancelled)
    }

    /// Returns true when Tree-sitter rejected the Achitek grammar.
    ///
    /// See [`parse_tree`] for a complete example.
    pub fn is_language(&self) -> bool {
        matches!(self.kind, ParseErrorKind::Language(_))
    }

    /// Returns the upstream Tree-sitter language error, if one occurred.
    ///
    /// See [`parse_tree`] for a complete example.
    pub fn language_error(&self) -> Option<&tree_sitter::LanguageError> {
        match &self.kind {
            ParseErrorKind::Language(source) => Some(source),
            ParseErrorKind::ParseCancelled => None,
        }
    }

    /// Returns the backtrace captured when the error was created.
    ///
    /// See [`parse_tree`] for a complete example.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
}

impl From<tree_sitter::LanguageError> for ParseError {
    fn from(source: tree_sitter::LanguageError) -> Self {
        Self::language(source)
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ParseErrorKind::Language(source) => {
                writeln!(f, "failed to configure the Achitek parser: {source}")?;
            }
            ParseErrorKind::ParseCancelled => {
                writeln!(f, "tree-sitter did not produce a parse tree")?;
            }
        }

        write!(f, "backtrace:\n{}", self.backtrace)
    }
}

impl StdError for ParseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.kind {
            ParseErrorKind::Language(source) => Some(source),
            ParseErrorKind::ParseCancelled => None,
        }
    }
}

#[derive(Debug)]
enum ParseErrorKind {
    Language(tree_sitter::LanguageError),
    ParseCancelled,
}
