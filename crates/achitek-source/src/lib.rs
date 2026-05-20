//! Shared source-analysis primitives for Achitek crates.
//!
//! This crate intentionally stays small. It contains cross-language plumbing
//! such as byte positions, source ranges, spanned values, diagnostic severity,
//! and Tree-sitter range helpers. Domain-specific models and diagnostic codes
//! belong in the language crates that define those contracts.
//!
//! # Example
//!
//! ```
//! let range = achitek_source::TextRange {
//!     start: achitek_source::TextPosition { line: 0, byte: 0 },
//!     end: achitek_source::TextPosition { line: 0, byte: 7 },
//! };
//! let spanned = achitek_source::Spanned {
//!     value: "project",
//!     range,
//! };
//!
//! assert_eq!(spanned.as_ref(), &"project");
//! assert_eq!(spanned.range.start.line, 0);
//! ```

#![deny(missing_docs)]

use tree_sitter::{Node, Point, Range};

/// Severity level for a source diagnostic.
///
/// Severity indicates how tools should present a diagnostic. Errors describe
/// invalid source that should prevent normal execution. Warnings describe
/// suspicious source that can still be analyzed. Hints provide low-priority
/// guidance.
///
/// ```
/// let severity = achitek_source::Severity::Warning;
///
/// assert_eq!(format!("{severity:?}"), "Warning");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    /// Invalid source that should prevent normal execution.
    Error,
    /// Suspicious source that can still be analyzed.
    Warning,
    /// Low-priority guidance.
    Hint,
}

/// A zero-based byte position in source text.
///
/// `line` and `byte` use Tree-sitter's native coordinate system: the line is
/// zero-based and `byte` is the zero-based UTF-8 byte offset from the beginning
/// of that line.
///
/// ```
/// let position = achitek_source::TextPosition { line: 2, byte: 4 };
///
/// assert_eq!(position.line, 2);
/// assert_eq!(position.byte, 4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextPosition {
    /// Zero-based line number.
    pub line: usize,

    /// Zero-based UTF-8 byte offset within the line.
    pub byte: usize,
}

impl From<Point> for TextPosition {
    fn from(point: Point) -> Self {
        Self {
            line: point.row,
            byte: point.column,
        }
    }
}

/// A byte range in source text.
///
/// The range starts at `start` and ends at `end`, both expressed as zero-based
/// line plus UTF-8 byte offset positions. Consumers can convert this into their
/// presentation protocol's expected position encoding.
///
/// ```
/// let range = achitek_source::TextRange {
///     start: achitek_source::TextPosition { line: 0, byte: 0 },
///     end: achitek_source::TextPosition { line: 0, byte: 4 },
/// };
///
/// assert_eq!(range.start.byte, 0);
/// assert_eq!(range.end.byte, 4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextRange {
    /// Start position of the range.
    pub start: TextPosition,

    /// End position of the range.
    pub end: TextPosition,
}

impl From<Range> for TextRange {
    fn from(range: Range) -> Self {
        Self {
            start: range.start_point.into(),
            end: range.end_point.into(),
        }
    }
}

/// A value paired with the source range that produced it.
///
/// Spans let editor-facing consumers connect recovered model values back to the
/// original source text without exposing Tree-sitter nodes.
///
/// ```
/// let spanned = achitek_source::Spanned {
///     value: "name",
///     range: achitek_source::TextRange::default(),
/// };
///
/// assert_eq!(spanned.as_ref(), &"name");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spanned<T> {
    /// Recovered model value.
    pub value: T,
    /// Source range that produced the value.
    pub range: TextRange,
}

impl<T> AsRef<T> for Spanned<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T> AsMut<T> for Spanned<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

/// Returns named children for a Tree-sitter node.
///
/// ```
/// # fn example(node: tree_sitter::Node<'_>) {
/// let named_kinds = achitek_source::named_children(node)
///     .map(|child| child.kind())
///     .collect::<Vec<_>>();
/// # let _ = named_kinds;
/// # }
/// ```
pub fn named_children(node: Node<'_>) -> std::vec::IntoIter<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .collect::<Vec<_>>()
        .into_iter()
}

/// Returns the UTF-8 source text covered by a Tree-sitter node.
///
/// Tree-sitter byte ranges are expected to align with the original source text
/// supplied to the parser. A panic here indicates the parser tree and source
/// text no longer belong together.
///
/// ```
/// # fn example(node: tree_sitter::Node<'_>, source: &str) {
/// let source_text = achitek_source::text(node, source);
/// # let _ = source_text;
/// # }
/// ```
pub fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes())
        .expect("tree-sitter node byte ranges should be valid utf-8 slices")
}

/// Converts a Tree-sitter node range into a source text range.
///
/// ```
/// # fn example(node: tree_sitter::Node<'_>) {
/// let range = achitek_source::text_range_for_node(node);
///
/// assert!(range.start <= range.end);
/// # }
/// ```
pub fn text_range_for_node(node: Node<'_>) -> TextRange {
    TextRange {
        start: node.start_position().into(),
        end: node.end_position().into(),
    }
}

/// Returns true when `node` is the child assigned to `field_name` on `parent`.
///
/// ```
/// # fn example(parent: tree_sitter::Node<'_>, child: tree_sitter::Node<'_>) {
/// if achitek_source::is_child_for_field(parent, child, "name") {
///     assert_eq!(parent.child_by_field_name("name"), Some(child));
/// }
/// # }
/// ```
pub fn is_child_for_field(parent: Node<'_>, node: Node<'_>, field_name: &str) -> bool {
    parent
        .child_by_field_name(field_name)
        .is_some_and(|child| child == node)
}
