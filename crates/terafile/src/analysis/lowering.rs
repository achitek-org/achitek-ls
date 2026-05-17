//! Lowers the Tree-sitter syntax tree into the recovering Tera model.
//!
//! This module walks the parsed template and records semantic facts that are
//! useful to editor tooling: dependencies, macro definitions, bindings,
//! variable references, and callable references. It does not decide whether
//! those facts are valid; diagnostics can use the same source tree and model to
//! explain violations with better source locations.

use super::syntax::{is_child_for_field, named_children, text, text_range_for_node};
use crate::model::{
    Binding, BindingKind, Macro, MacroCall, MacroParameter, NamedReference, Spanned,
    TemplateDependency, TemplateDependencyKind, TemplatePath, TeraFile, VariableReference,
};
use tree_sitter::{Node, Tree};

impl TeraFile {
    /// Creates a recovering Tera model from a parsed Tree-sitter tree.
    pub(crate) fn from_tree(tree: &Tree, source: &str) -> Self {
        let mut builder = TeraFileBuilder::default();
        builder.visit(tree.root_node(), source);
        builder.finish()
    }
}

#[derive(Default)]
struct TeraFileBuilder {
    dependencies: Vec<Spanned<TemplateDependency>>,
    macros: Vec<Spanned<Macro>>,
    bindings: Vec<Spanned<Binding>>,
    variable_references: Vec<Spanned<VariableReference>>,
    filters: Vec<Spanned<NamedReference>>,
    tests: Vec<Spanned<NamedReference>>,
    functions: Vec<Spanned<NamedReference>>,
    macro_calls: Vec<Spanned<MacroCall>>,
}

impl TeraFileBuilder {
    fn finish(self) -> TeraFile {
        TeraFile::new(
            self.dependencies,
            self.macros,
            self.bindings,
            self.variable_references,
            self.filters,
            self.tests,
            self.functions,
            self.macro_calls,
        )
    }

    fn visit(&mut self, node: Node<'_>, source: &str) {
        match node.kind() {
            "extends_statement" => self.record_extends(node, source),
            "include_statement" => self.record_include(node, source),
            "import_statement" => self.record_import(node, source),
            "macro_statement" => self.record_macro(node, source),
            "set_statement" => self.record_set(node, source),
            "for_statement" => self.record_for(node, source),
            "filter_statement" => self.record_filter_statement(node, source),
            "filter_expression" => self.record_filter_expression(node, source),
            "test_expression" => self.record_test_expression(node, source),
            "call_expression" => self.record_call_expression(node, source),
            "identifier" => self.record_identifier_reference(node, source),
            _ => {}
        }

        for child in named_children(node) {
            self.visit(child, source);
        }
    }

    fn record_extends(&mut self, node: Node<'_>, source: &str) {
        let Some(path) = first_string_child(node, source) else {
            return;
        };

        self.dependencies.push(Spanned {
            value: TemplateDependency {
                kind: TemplateDependencyKind::Extends,
                path: TemplatePath::Single(path),
            },
            range: text_range_for_node(node),
        });
    }

    fn record_include(&mut self, node: Node<'_>, source: &str) {
        let Some(path) = template_path_child(node, source) else {
            return;
        };

        self.dependencies.push(Spanned {
            value: TemplateDependency {
                kind: TemplateDependencyKind::Include {
                    ignore_missing: text(node, source).contains("ignore missing"),
                },
                path,
            },
            range: text_range_for_node(node),
        });
    }

    fn record_import(&mut self, node: Node<'_>, source: &str) {
        let Some(path) = first_string_child(node, source) else {
            return;
        };
        let namespace = node
            .child_by_field_name("scope")
            .map(|scope| text(scope, source).to_owned());

        if let Some(namespace) = namespace.as_ref() {
            self.bindings.push(Spanned {
                value: Binding {
                    name: namespace.clone(),
                    kind: BindingKind::ImportNamespace,
                },
                range: node
                    .child_by_field_name("scope")
                    .map(text_range_for_node)
                    .unwrap_or_else(|| text_range_for_node(node)),
            });
        }

        self.dependencies.push(Spanned {
            value: TemplateDependency {
                kind: TemplateDependencyKind::Import { namespace },
                path: TemplatePath::Single(path),
            },
            range: text_range_for_node(node),
        });
    }

    fn record_macro(&mut self, node: Node<'_>, source: &str) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let parameters = node
            .child_by_field_name("parameters")
            .map(|parameters| parse_macro_parameters(parameters, source))
            .unwrap_or_default();

        for parameter in &parameters {
            self.bindings.push(Spanned {
                value: Binding {
                    name: parameter.value.name.clone(),
                    kind: BindingKind::MacroParameter,
                },
                range: parameter.range,
            });
        }

        self.macros.push(Spanned {
            value: Macro {
                name: text(name_node, source).to_owned(),
                parameters,
            },
            range: text_range_for_node(node),
        });
    }

    fn record_set(&mut self, node: Node<'_>, source: &str) {
        let is_global = text(node, source).trim_start().starts_with("{% set_global");
        for child in named_children(node).filter(|child| child.kind() == "assignment_expression") {
            let Some(left) = child.child_by_field_name("left") else {
                continue;
            };
            if left.kind() != "identifier" {
                continue;
            }

            self.bindings.push(Spanned {
                value: Binding {
                    name: text(left, source).to_owned(),
                    kind: if is_global {
                        BindingKind::SetGlobal
                    } else {
                        BindingKind::Set
                    },
                },
                range: text_range_for_node(left),
            });
        }
    }

    fn record_for(&mut self, node: Node<'_>, source: &str) {
        let mut cursor = node.walk();
        for child in node.children_by_field_name("left", &mut cursor) {
            if child.kind() != "identifier" {
                continue;
            }

            self.bindings.push(Spanned {
                value: Binding {
                    name: text(child, source).to_owned(),
                    kind: BindingKind::ForVariable,
                },
                range: text_range_for_node(child),
            });
        }

        self.bindings.push(Spanned {
            value: Binding {
                name: "loop".to_owned(),
                kind: BindingKind::LoopVariable,
            },
            range: text_range_for_node(node),
        });
    }

    fn record_filter_statement(&mut self, node: Node<'_>, source: &str) {
        if let Some(filter) = named_children(node).find(|child| child.kind() == "identifier") {
            self.filters.push(named_reference(filter, source));
        }
    }

    fn record_filter_expression(&mut self, node: Node<'_>, source: &str) {
        if let Some(filter) = node.child_by_field_name("filter") {
            self.filters.push(named_reference(filter, source));
        }
    }

    fn record_test_expression(&mut self, node: Node<'_>, source: &str) {
        if let Some(test) = node.child_by_field_name("test") {
            self.tests.push(named_reference(test, source));
        }
    }

    fn record_call_expression(&mut self, node: Node<'_>, source: &str) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };

        let arguments = node
            .child_by_field_name("arguments")
            .map(|arguments| parse_keyword_arguments(arguments, source))
            .unwrap_or_default();

        if let Some(scope) = node.child_by_field_name("scope") {
            self.macro_calls.push(Spanned {
                value: MacroCall {
                    namespace: text(scope, source).to_owned(),
                    name: text(name_node, source).to_owned(),
                    arguments,
                },
                range: text_range_for_node(node),
            });
        } else {
            self.functions.push(named_reference(name_node, source));
        }
    }

    fn record_identifier_reference(&mut self, node: Node<'_>, source: &str) {
        if !is_variable_reference(node) {
            return;
        }

        let reference_node = reference_node(node);
        let path = text(reference_node, source).to_owned();
        let root = text(node, source).to_owned();

        self.variable_references.push(Spanned {
            value: VariableReference { path, root },
            range: text_range_for_node(reference_node),
        });
    }
}

fn parse_macro_parameters(node: Node<'_>, source: &str) -> Vec<Spanned<MacroParameter>> {
    let mut parameters = Vec::new();
    let mut cursor = node.walk();

    for parameter in node.children_by_field_name("parameter", &mut cursor) {
        parameters.push(Spanned {
            value: MacroParameter {
                name: text(parameter, source).to_owned(),
                has_default: false,
            },
            range: text_range_for_node(parameter),
        });
    }

    for optional in named_children(node).filter(|child| child.kind() == "optional_parameter") {
        let Some(name) = optional.child_by_field_name("name") else {
            continue;
        };

        parameters.push(Spanned {
            value: MacroParameter {
                name: text(name, source).to_owned(),
                has_default: true,
            },
            range: text_range_for_node(name),
        });
    }

    parameters
}

fn parse_keyword_arguments(node: Node<'_>, source: &str) -> Vec<Spanned<NamedReference>> {
    named_children(node)
        .filter(|child| child.kind() == "keyword_argument")
        .filter_map(|argument| {
            let name = argument.child_by_field_name("name")?;
            Some(named_reference(name, source))
        })
        .collect()
}

fn first_string_child(node: Node<'_>, source: &str) -> Option<String> {
    named_children(node)
        .find(|child| child.kind() == "string")
        .and_then(|string| parse_string(string, source))
}

fn template_path_child(node: Node<'_>, source: &str) -> Option<TemplatePath> {
    let child =
        named_children(node).find(|child| child.kind() == "string" || child.kind() == "array")?;
    match child.kind() {
        "string" => parse_string(child, source).map(TemplatePath::Single),
        "array" => {
            let paths = named_children(child)
                .filter(|item| item.kind() == "string")
                .filter_map(|item| parse_string(item, source))
                .collect::<Vec<_>>();
            Some(TemplatePath::Choice(paths))
        }
        _ => None,
    }
}

fn parse_string(node: Node<'_>, source: &str) -> Option<String> {
    let raw = text(node, source);
    let mut chars = raw.chars();
    let quote = chars.next()?;
    if !matches!(quote, '"' | '\'' | '`') || !raw.ends_with(quote) {
        return None;
    }

    Some(raw[quote.len_utf8()..raw.len() - quote.len_utf8()].to_owned())
}

fn named_reference(node: Node<'_>, source: &str) -> Spanned<NamedReference> {
    Spanned {
        value: NamedReference {
            name: text(node, source).to_owned(),
        },
        range: text_range_for_node(node),
    }
}

fn is_variable_reference(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return true;
    };

    match parent.kind() {
        "call_expression" => {
            !is_child_for_field(parent, node, "name") && !is_child_for_field(parent, node, "scope")
        }
        "filter_expression" => !is_child_for_field(parent, node, "filter"),
        "test_expression" => !is_child_for_field(parent, node, "test"),
        "import_statement" => !is_child_for_field(parent, node, "scope"),
        "keyword_argument" | "optional_parameter" => !is_child_for_field(parent, node, "name"),
        "macro_statement" => !is_child_for_field(parent, node, "name"),
        "assignment_expression" => !is_child_for_field(parent, node, "left"),
        "for_statement" => !is_child_for_field(parent, node, "left"),
        "member_expression" => {
            !is_child_for_field(parent, node, "property")
                && !is_child_for_field(parent, node, "index")
        }
        _ => true,
    }
}

fn reference_node(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() != "member_expression" || !is_child_for_field(parent, current, "value") {
            break;
        }
        current = parent;
    }

    current
}
