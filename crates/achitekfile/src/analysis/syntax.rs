use crate::{TextPosition, TextRange};
use tree_sitter::{Node, Point};

pub(super) fn named_children(node: Node<'_>) -> std::vec::IntoIter<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .collect::<Vec<_>>()
        .into_iter()
}

pub(super) fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes())
        .expect("tree-sitter node byte ranges should be valid utf-8 slices")
}

pub(super) fn text_range_for_node(node: Node<'_>) -> TextRange {
    TextRange {
        start: text_position_for_point(node.start_position()),
        end: text_position_for_point(node.end_position()),
    }
}

fn text_position_for_point(point: Point) -> TextPosition {
    TextPosition {
        line: point.row,
        byte: point.column,
    }
}
