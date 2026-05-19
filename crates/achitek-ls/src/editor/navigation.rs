use super::{DefinitionTarget, PrepareRenameTarget, ReferenceTarget, SourceTree, shared};
use achitekfile::{TextPosition, TextRange};
use tree_sitter::Node;

/// Prompt declaration ranges used by editor features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PromptDeclaration {
    name: String,
    range: TextRange,
    selection_range: TextRange,
}

pub fn collect_prompt_declarations(
    syntax: &SourceTree,
    analysis: &achitekfile::Analysis<'_>,
) -> Vec<PromptDeclaration> {
    analysis
        .file()
        .prompts()
        .iter()
        .map(|prompt| {
            let selection_range = shared::prompt_block_for_range(syntax, prompt.range)
                .and_then(|node| node.child_by_field_name("name"))
                .map(|node| syntax.range_for(node))
                .unwrap_or(prompt.range);

            PromptDeclaration {
                name: prompt.value.name.clone(),
                range: prompt.range,
                selection_range,
            }
        })
        .collect()
}

pub fn definition_for_position(
    syntax: &SourceTree,
    prompt_declarations: &[PromptDeclaration],
    position: TextPosition,
) -> Option<DefinitionTarget> {
    let point = tree_sitter::Point {
        row: position.line,
        column: position.byte,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;
    let reference_name = match node.kind() {
        "identifier" => identifier_reference_name(syntax, node),
        _ => None,
    }?;

    let declaration = prompt_declarations
        .iter()
        .find(|declaration| declaration.name == reference_name)?;

    Some(DefinitionTarget {
        range: declaration.range,
        selection_range: declaration.selection_range,
    })
}

pub fn prepare_rename_for_position(
    syntax: &SourceTree,
    prompt_declarations: &[PromptDeclaration],
    position: TextPosition,
) -> Option<PrepareRenameTarget> {
    let point = tree_sitter::Point {
        row: position.line,
        column: position.byte,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;

    match node.kind() {
        "identifier" => {
            let name = identifier_reference_name(syntax, node)?;
            prompt_declarations
                .iter()
                .find(|declaration| declaration.name == name)?;
            Some(PrepareRenameTarget {
                range: syntax.range_for(node),
                placeholder: name.to_owned(),
            })
        }
        "prompt_block" | "string_literal" => {
            let declaration = declaration_at_position(prompt_declarations, position)?;
            Some(PrepareRenameTarget {
                range: declaration.selection_range,
                placeholder: declaration.name.clone(),
            })
        }
        _ => None,
    }
}

pub fn references_for_position(
    syntax: &SourceTree,
    prompt_declarations: &[PromptDeclaration],
    position: TextPosition,
    include_declaration: bool,
) -> Vec<ReferenceTarget> {
    let Some(name) = prompt_name_at_position(syntax, position, prompt_declarations) else {
        return Vec::new();
    };

    let mut references = Vec::new();

    if include_declaration
        && let Some(declaration) = prompt_declarations
            .iter()
            .find(|declaration| declaration.name == name)
    {
        references.push(ReferenceTarget {
            range: declaration.selection_range,
        });
    }

    collect_reference_nodes(syntax.root_node(), syntax, &name, &mut references);
    references
}

pub fn prompt_name_at_position(
    syntax: &SourceTree,
    position: TextPosition,
    prompt_declarations: &[PromptDeclaration],
) -> Option<String> {
    let point = tree_sitter::Point {
        row: position.line,
        column: position.byte,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;

    match node.kind() {
        "identifier" => {
            let name = identifier_reference_name(syntax, node)?;
            prompt_declarations
                .iter()
                .find(|declaration| declaration.name == name)?;
            Some(name.to_owned())
        }
        "prompt_block" | "string_literal" => declaration_at_position(prompt_declarations, position)
            .map(|declaration| declaration.name.clone()),
        _ => None,
    }
}

fn declaration_at_position(
    prompt_declarations: &[PromptDeclaration],
    position: TextPosition,
) -> Option<&PromptDeclaration> {
    prompt_declarations
        .iter()
        .find(|declaration| range_contains_position(declaration.selection_range, position))
}

fn range_contains_position(range: TextRange, position: TextPosition) -> bool {
    range.start <= position && position <= range.end
}

fn collect_reference_nodes(
    node: Node<'_>,
    syntax: &SourceTree,
    target_name: &str,
    references: &mut Vec<ReferenceTarget>,
) {
    if node.kind() == "identifier"
        && identifier_reference_name(syntax, node).is_some_and(|name| name == target_name)
    {
        references.push(ReferenceTarget {
            range: syntax.range_for(node),
        });
    }

    for index in 0..node.child_count() {
        let Some(child) =
            node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };
        collect_reference_nodes(child, syntax, target_name, references);
    }
}

fn identifier_reference_name<'a>(syntax: &'a SourceTree, node: Node<'_>) -> Option<&'a str> {
    let parent = node.parent()?;
    let is_reference_site = match parent.kind() {
        "simple_dependency" => parent.child_by_field_name("reference") == Some(node),
        "comparison_dependency" => parent.child_by_field_name("left") == Some(node),
        "method_call_dependency" => parent.child_by_field_name("receiver") == Some(node),
        _ => false,
    };

    if is_reference_site {
        Some(syntax.text_for(node))
    } else {
        None
    }
}
