//! Diagnostic collection for Tera analysis.

use super::syntax::{text, text_range_for_node};
use crate::{Diagnostic, DiagnosticCode};
use tree_sitter::{Node, Tree};

/// Collects diagnostics for one parsed Tera syntax tree.
pub(super) fn collect_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_syntax_diagnostics(tree.root_node(), source, &mut diagnostics);
    collect_semantic_diagnostics(tree.root_node(), source, &mut diagnostics);
    diagnostics
}

fn collect_syntax_diagnostics(node: Node<'_>, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.is_missing() {
        diagnostics.push(Diagnostic::new(
            missing_node_code(node),
            text_range_for_node(node),
        ));
        return;
    }

    if node.is_error() {
        diagnostics.push(Diagnostic::new(
            error_node_code(node, source),
            text_range_for_node(node),
        ));
        return;
    }

    for child in children(node) {
        collect_syntax_diagnostics(child, source, diagnostics);
    }
}

fn missing_node_code(node: Node<'_>) -> DiagnosticCode {
    match node.kind() {
        "endif" | "endfor" | "endblock" | "endmacro" | "endfilter" | "endraw" => {
            DiagnosticCode::UnexpectedEndTag
        }
        "}}" | "%}" | "-}}" | "-%}" | "#}" | "-#}" => DiagnosticCode::UnterminatedTag,
        _ => DiagnosticCode::SyntaxError,
    }
}

fn error_node_code(node: Node<'_>, source: &str) -> DiagnosticCode {
    let text = text(node, source);

    if starts_with_dynamic_include(text) {
        return DiagnosticCode::DynamicIncludePath;
    }

    if starts_with_any_end_tag(text) {
        return DiagnosticCode::UnexpectedEndTag;
    }

    if contains_mismatched_end_tag(text) {
        return DiagnosticCode::MismatchedEndTag;
    }

    if has_unclosed_tag_start(text) {
        return DiagnosticCode::UnterminatedTag;
    }

    DiagnosticCode::SyntaxError
}

fn collect_semantic_diagnostics(root: Node<'_>, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    collect_source_macro_diagnostics(root, source, diagnostics);
    collect_extends_diagnostics(root, source, diagnostics);
    collect_macro_diagnostics(root, diagnostics);
    collect_error_macro_diagnostics(root, source, diagnostics);
    collect_static_include_diagnostics(root, diagnostics);
}

fn collect_source_macro_diagnostics(
    root: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if tag_appears_inside_macro(source, "{% block") {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::BlockNotAllowedInMacro,
            text_range_for_node(root),
        ));
    }

    if tag_appears_inside_macro(source, "{% extends") {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::ExtendsNotAllowedInMacro,
            text_range_for_node(root),
        ));
    }
}

fn collect_extends_diagnostics(root: Node<'_>, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut saw_renderable_item = false;
    let mut saw_extends = false;

    for child in named_children(root) {
        match child.kind() {
            "comment_tag" | "frontmatter" => {}
            "content" if text(child, source).trim().is_empty() => {}
            "extends_statement" => {
                saw_extends = true;
                if saw_renderable_item {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ExtendsNotFirst,
                        text_range_for_node(child),
                    ));
                }
            }
            "content" if saw_extends && !text(child, source).trim().is_empty() => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ContentOutsideBlockInChildTemplate,
                    text_range_for_node(child),
                ));
            }
            "content" => {
                saw_renderable_item = true;
            }
            _ if !saw_extends => {
                saw_renderable_item = true;
            }
            _ => {}
        }
    }
}

fn collect_macro_diagnostics(root: Node<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for node in named_descendants(root) {
        if node.is_error() {
            continue;
        }

        if node.kind() != "macro_statement" {
            continue;
        }

        if node
            .parent()
            .is_some_and(|parent| parent.kind() != "source_file")
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MacroNotTopLevel,
                text_range_for_node(node),
            ));
        }

        for child in named_descendants(node) {
            match child.kind() {
                "block_statement" => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::BlockNotAllowedInMacro,
                    text_range_for_node(child),
                )),
                "extends_statement" => diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ExtendsNotAllowedInMacro,
                    text_range_for_node(child),
                )),
                _ => {}
            }
        }
    }
}

fn collect_error_macro_diagnostics(
    root: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in descendants(root).filter(|node| node.is_error()) {
        let text = text(node, source);
        if text.contains("{% macro") && text.contains("{% block") {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::BlockNotAllowedInMacro,
                text_range_for_node(node),
            ));
        }

        if text.contains("{% macro") && text.contains("{% extends") {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ExtendsNotAllowedInMacro,
                text_range_for_node(node),
            ));
        }
    }
}

fn collect_static_include_diagnostics(root: Node<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for node in named_descendants(root)
        .into_iter()
        .filter(|node| node.kind() == "include_statement")
    {
        let has_static_path =
            named_children(node).any(|child| matches!(child.kind(), "string" | "array"));

        if !has_static_path {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DynamicIncludePath,
                text_range_for_node(node),
            ));
        }
    }
}

fn has_unclosed_tag_start(text: &str) -> bool {
    let opens =
        text.matches("{{").count() + text.matches("{%").count() + text.matches("{#").count();
    let closes =
        text.matches("}}").count() + text.matches("%}").count() + text.matches("#}").count();

    opens > closes
}

fn starts_with_any_end_tag(text: &str) -> bool {
    let trimmed = text.trim_start();
    [
        "{% endif",
        "{% endfor",
        "{% endblock",
        "{% endmacro",
        "{% endfilter",
        "{% endraw",
    ]
    .iter()
    .any(|tag| trimmed.starts_with(tag))
}

fn starts_with_dynamic_include(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("{% include")
        && !trimmed.contains('"')
        && !trimmed.contains('\'')
        && !trimmed.contains('`')
}

fn contains_mismatched_end_tag(text: &str) -> bool {
    let has_open = [
        "{% if",
        "{% for",
        "{% block",
        "{% macro",
        "{% filter",
        "{% raw",
    ]
    .iter()
    .any(|tag| text.contains(tag));
    let has_end = [
        "{% endif",
        "{% endfor",
        "{% endblock",
        "{% endmacro",
        "{% endfilter",
        "{% endraw",
    ]
    .iter()
    .any(|tag| text.contains(tag));

    has_open && has_end
}

fn tag_appears_inside_macro(source: &str, tag: &str) -> bool {
    let mut remaining = source;

    while let Some(macro_start) = remaining.find("{% macro") {
        remaining = &remaining[macro_start..];
        let Some(macro_end) = remaining.find("{% endmacro") else {
            return remaining.contains(tag);
        };

        if remaining[..macro_end].contains(tag) {
            return true;
        }

        remaining = &remaining[macro_end + "{% endmacro".len()..];
    }

    false
}

fn children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect::<Vec<_>>().into_iter()
}

fn descendants(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut descendants = Vec::new();
    collect_descendants(node, &mut descendants);
    descendants.into_iter()
}

fn collect_descendants<'tree>(node: Node<'tree>, descendants: &mut Vec<Node<'tree>>) {
    for child in children(node) {
        descendants.push(child);
        collect_descendants(child, descendants);
    }
}

fn named_children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .collect::<Vec<_>>()
        .into_iter()
}

fn named_descendants(node: Node<'_>) -> Vec<Node<'_>> {
    let mut descendants = Vec::new();
    collect_named_descendants(node, &mut descendants);
    descendants
}

fn collect_named_descendants<'tree>(node: Node<'tree>, descendants: &mut Vec<Node<'tree>>) {
    for child in named_children(node) {
        descendants.push(child);
        collect_named_descendants(child, descendants);
    }
}
