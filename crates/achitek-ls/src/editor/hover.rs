use super::{SourceTree, shared};
use achitekfile::{TextPosition, TextRange};
use tree_sitter::Node;

/// Hover content derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    contents: String,
    range: TextRange,
}

impl Hover {
    /// Returns the hover contents as markdown-friendly text.
    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// Returns the range that should be highlighted for the hover.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

pub fn hover_for_position(syntax: &SourceTree, position: TextPosition) -> Option<Hover> {
    let point = tree_sitter::Point {
        row: position.line,
        column: position.byte,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;

    let hover = match node.kind() {
        "prompt_block" => hover_for_prompt_block(syntax, node),
        "blueprint_block" => Some(simple_hover(
            syntax.range_for(node),
            "## blueprint\n\nDeclares top-level blueprint metadata for the Achitekfile.",
        )),
        "blueprint_attribute_key" => hover_for_blueprint_attribute_key(syntax, node),
        "type_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `type`\n\nDeclares the prompt type. Valid values include `string`, `paragraph`, `bool`, `select`, and `multiselect`.",
        )),
        "help_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `help`\n\nProvides the human-readable prompt text shown to the user.",
        )),
        "choices_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `choices`\n\nLists selectable values for `select` and `multiselect` prompts.",
        )),
        "default_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `default`\n\nProvides the default answer for the prompt. The value should match the prompt type.",
        )),
        "required_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `required`\n\nControls whether the prompt must be answered. This is typically `true` unless optional input is allowed.",
        )),
        "depends_on_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `depends_on`\n\nControls whether a prompt is shown based on previous answers. It can reference other prompts directly or use comparison and combinator expressions.",
        )),
        "question_type" => hover_for_prompt_type(syntax, node),
        "validate_block" => Some(simple_hover(
            syntax.range_for(node),
            "## validate\n\nContains validation rules for the surrounding prompt, such as length limits or regex checks.",
        )),
        "regex_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `regex`\n\nRequires the prompt value to match the given regular expression.",
        )),
        "min_length_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `min_length`\n\nRequires at least this many characters for string-like prompts.",
        )),
        "max_length_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `max_length`\n\nLimits string-like prompts to at most this many characters.",
        )),
        "min_selections_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `min_selections`\n\nRequires at least this many selected values for a `multiselect` prompt.",
        )),
        "max_selections_attribute" => Some(simple_hover(
            syntax.range_for(node),
            "## `max_selections`\n\nLimits a `multiselect` prompt to at most this many selected values.",
        )),
        "combinator_name" => Some(simple_hover(
            syntax.range_for(node),
            "## dependency combinator\n\nCombines dependency conditions. `all(...)` requires every nested condition to match, while `any(...)` requires at least one.",
        )),
        "method_name" => Some(simple_hover(
            syntax.range_for(node),
            "## `contains`\n\nChecks whether a prompt value includes the given literal, commonly for `multiselect` prompts.",
        )),
        _ => None,
    };

    hover.or_else(|| {
        node.parent().and_then(|parent| match parent.kind() {
            "prompt_block" => hover_for_prompt_block(syntax, parent),
            _ => None,
        })
    })
}

fn hover_for_prompt_block(syntax: &SourceTree, node: Node<'_>) -> Option<Hover> {
    let name_node = node.child_by_field_name("name")?;
    let name = syntax.text_for(name_node).trim_matches('"');
    let prompt_type = shared::prompt_type_for_block(syntax, node).unwrap_or("unknown");

    Some(simple_hover(
        syntax.range_for(name_node),
        format!(
            "## prompt `{name}`\n\nType: `{prompt_type}`\n\nDefines an interactive prompt in the Achitekfile."
        ),
    ))
}

fn hover_for_blueprint_attribute_key(syntax: &SourceTree, node: Node<'_>) -> Option<Hover> {
    let key = syntax.text_for(node);
    let description = match key {
        "version" => "Declares the Achitekfile schema version.",
        "name" => "Provides the blueprint identifier.",
        "description" => "Provides a human-readable blueprint description.",
        "author" => "Records the blueprint author.",
        "min_achitek_version" => {
            "Declares the minimum Achitek version required for this blueprint."
        }
        _ => return None,
    };

    Some(simple_hover(
        syntax.range_for(node),
        format!("## `{key}`\n\n{description}"),
    ))
}

fn hover_for_prompt_type(syntax: &SourceTree, node: Node<'_>) -> Option<Hover> {
    let prompt_type = syntax.text_for(node);
    let description = match prompt_type {
        "string" => "A single-line text prompt.",
        "paragraph" => "A multi-line text prompt.",
        "bool" => "A boolean yes/no prompt.",
        "select" => "A single-choice prompt from a list of options.",
        "multiselect" => "A prompt that allows selecting multiple values.",
        _ => return None,
    };

    Some(simple_hover(
        syntax.range_for(node),
        format!("## `{prompt_type}`\n\n{description}"),
    ))
}

fn simple_hover(range: TextRange, contents: impl Into<String>) -> Hover {
    Hover {
        contents: contents.into(),
        range,
    }
}
