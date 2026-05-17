//! Syntax
use thiserror::Error;
use tree_sitter::{Node, Parser, Point, Tree};

/// Syntax tree for a single Achitek source document.
///
/// This type wraps the raw Tree-sitter parse tree together with the original
/// source text and any syntax issues collected during parsing.
#[derive(Debug)]
pub struct SyntaxTree {
    source: String,
    tree: Tree,
    errors: Vec<SyntaxError>,
}

impl SyntaxTree {
    /// Returns the original source used to build this syntax tree.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the raw Tree-sitter tree.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Returns the root CST node.
    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    /// Returns the source range occupied by a given node.
    pub fn range_for(&self, node: Node<'_>) -> TextRange {
        TextRange::from_node(node)
    }

    /// Returns the source text covered by a given node.
    pub fn text_for<'a>(&'a self, node: Node<'_>) -> &'a str {
        &self.source[node.byte_range()]
    }

    /// Returns true when Tree-sitter reported any syntax issues.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns the syntax issues discovered while parsing.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }
}

/// A syntax issue reported by Tree-sitter.
///
/// These issues are derived from Tree-sitter recovery nodes so callers can
/// inspect malformed input without dealing with Tree-sitter internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    kind: SyntaxErrorKind,
    range: TextRange,
}

impl SyntaxError {
    /// Returns the kind of syntax issue.
    pub fn kind(&self) -> SyntaxErrorKind {
        self.kind
    }

    /// Returns the source range that Tree-sitter associated with the issue.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

/// The kind of syntax issue reported by Tree-sitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxErrorKind {
    /// Tree-sitter inserted a missing node to recover from malformed input.
    Missing,
    /// Tree-sitter produced an explicit error node for malformed input.
    Unexpected,
}

/// A line/column position in source text.
///
/// Positions use Tree-sitter's row/column coordinates, which are zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    /// Zero-based line number.
    pub row: usize,
    /// Zero-based column within the line.
    pub column: usize,
}

impl From<Point> for TextPosition {
    fn from(point: Point) -> Self {
        Self {
            row: point.row,
            column: point.column,
        }
    }
}

/// A byte and position range in source text.
///
/// This stores both byte offsets and row/column positions so later layers can
/// map syntax issues into editor-facing ranges without recomputing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    /// Start offset in bytes from the beginning of the source.
    pub start_byte: usize,
    /// End offset in bytes from the beginning of the source.
    pub end_byte: usize,
    /// Start position using zero-based row/column coordinates.
    pub start_position: TextPosition,
    /// End position using zero-based row/column coordinates.
    pub end_position: TextPosition,
}

impl TextRange {
    fn from_node(node: Node<'_>) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_position: node.start_position().into(),
            end_position: node.end_position().into(),
        }
    }
}

/// Errors that can occur while building a syntax tree.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The Achitek grammar could not be installed into the parser.
    #[error("failed to configure the Achitek parser: {0}")]
    Language(#[from] tree_sitter::LanguageError),
    /// Parsing was interrupted before Tree-sitter produced a tree.
    #[error("tree-sitter did not produce a parse tree")]
    ParseCancelled,
}

/// Parses Achitek source text into a syntax tree.
///
/// This is the main entry point for the `syntax` crate. It configures a fresh
/// Tree-sitter parser with the Achitek grammar, parses the provided source,
/// and collects recoverable syntax issues from the resulting CST.
pub fn parse(source: &str) -> Result<SyntaxTree, ParseError> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_achitekfile::LANGUAGE.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or(ParseError::ParseCancelled)?;
    let errors = collect_errors(tree.root_node());

    Ok(SyntaxTree {
        source: source.to_owned(),
        tree,
        errors,
    })
}

/// Walks the parse tree and collects syntax issues reported by Tree-sitter.
fn collect_errors(root: Node<'_>) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    collect_errors_from_node(root, &mut errors);
    errors
}

/// Recursively visits a node and records recoverable syntax issues.
///
/// The returned boolean indicates whether this node or any of its descendants
/// contain a syntax error. That lets callers avoid reporting both a parent
/// error node and all of its nested error children as duplicate diagnostics.
fn collect_errors_from_node(node: Node<'_>, errors: &mut Vec<SyntaxError>) -> bool {
    let mut child_has_error = false;

    for index in 0..node.child_count() {
        let child = node
            .child(u32::try_from(index).expect("child index should fit into u32"))
            .expect("child index should be valid");
        child_has_error |= collect_errors_from_node(child, errors);
    }

    if node.is_missing() {
        errors.push(SyntaxError {
            kind: SyntaxErrorKind::Missing,
            range: TextRange::from_node(node),
        });
        return true;
    }

    if node.is_error() && !child_has_error {
        errors.push(SyntaxError {
            kind: SyntaxErrorKind::Unexpected,
            range: TextRange::from_node(node),
        });
        return true;
    }

    child_has_error || node.has_error()
}

#[cfg(test)]
mod tests {
    use super::{SyntaxErrorKind, parse};
    use indoc::indoc;

    #[test]
    fn parses_valid_achitek_source() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }
        "#};

        let tree = parse(source).expect("valid source should parse");

        assert_eq!(tree.root_node().kind(), "file");
        assert!(!tree.has_errors());
        assert!(tree.errors().is_empty());
    }

    #[test]
    fn reports_syntax_errors_for_invalid_source() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "broken"

            prompt "project_name" {
              type = string
            }
        "#};

        let tree = parse(source).expect("tree-sitter should still produce a tree");

        assert!(tree.has_errors());
        assert!(!tree.errors().is_empty());
        assert!(
            tree.errors()
                .iter()
                .any(|error| error.kind() == SyntaxErrorKind::Missing
                    || error.kind() == SyntaxErrorKind::Unexpected)
        );
    }
}
