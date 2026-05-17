//! Tree-sitter helper functions for Tera analysis.

use crate::{TextPosition, TextRange};
use tree_sitter::Node;

/// Returns the UTF-8 source text covered by a node.
pub(crate) fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Returns named children for a node.
pub(crate) fn named_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .collect::<Vec<_>>()
        .into_iter()
}

/// Converts a Tree-sitter node range into this crate's text range type.
pub(crate) fn text_range_for_node(node: Node<'_>) -> TextRange {
    TextRange {
        start: TextPosition::from(node.start_position()),
        end: TextPosition::from(node.end_position()),
    }
}

/// Returns true when `node` is the child assigned to `field_name` on `parent`.
pub(crate) fn is_child_for_field(parent: Node<'_>, node: Node<'_>, field_name: &str) -> bool {
    parent
        .child_by_field_name(field_name)
        .is_some_and(|child| child == node)
}
