use super::{SourceTree, Symbol, SymbolKind, shared};
use achitekfile::TextPosition;
use tree_sitter::Node;

/// Completion kinds understood by editor features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A language keyword or DSL construct.
    Keyword,
    /// A property or attribute name.
    Property,
    /// A value domain such as a prompt type.
    Value,
    /// A reference to another prompt.
    Reference,
    /// A built-in function or combinator.
    Function,
}

/// Completion item derived from Achitek source and context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    label: String,
    detail: Option<String>,
    kind: CompletionKind,
}

impl Completion {
    /// Returns the completion label inserted into the document.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns optional detail text for the completion item.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// Returns the completion kind.
    pub fn kind(&self) -> CompletionKind {
        self.kind
    }
}

pub(super) fn completions_for_position(
    syntax: &SourceTree,
    symbols: &[Symbol],
    position: TextPosition,
) -> Vec<Completion> {
    let line = source_line(syntax.source(), position.line);
    let prefix = prefix_before_column(line, position.byte);
    let trimmed = prefix.trim_start();

    if trimmed.starts_with("type") {
        return prompt_type_completions();
    }

    if trimmed.starts_with("depends_on") {
        return depends_on_completions(symbols);
    }

    if in_validate_block(syntax, position) {
        return validate_attribute_completions(syntax, position);
    }

    if in_prompt_block(syntax, position) {
        return prompt_attribute_completions(syntax, position);
    }

    if in_blueprint_block(syntax, position) {
        return blueprint_attribute_completions();
    }

    top_level_completions()
}

fn source_line(source: &str, row: usize) -> &str {
    source.lines().nth(row).unwrap_or("")
}

fn prefix_before_column(line: &str, column: usize) -> &str {
    let end = column.min(line.len());
    &line[..end]
}

fn in_prompt_block(syntax: &SourceTree, position: TextPosition) -> bool {
    ancestor_kinds_at_position(syntax, position).contains(&"prompt_block")
}

fn in_validate_block(syntax: &SourceTree, position: TextPosition) -> bool {
    ancestor_kinds_at_position(syntax, position).contains(&"validate_block")
}

fn in_blueprint_block(syntax: &SourceTree, position: TextPosition) -> bool {
    ancestor_kinds_at_position(syntax, position).contains(&"blueprint_block")
}

fn ancestor_kinds_at_position(syntax: &SourceTree, position: TextPosition) -> Vec<&str> {
    let point = tree_sitter::Point {
        row: position.line,
        column: position.byte,
    };
    let mut kinds = Vec::new();

    if let Some(mut node) = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)
    {
        loop {
            kinds.push(node.kind());
            let Some(parent) = node.parent() else {
                break;
            };
            node = parent;
        }
    }

    kinds
}

fn ancestor_node_at_position<'a>(
    syntax: &'a SourceTree,
    position: TextPosition,
    kind: &str,
) -> Option<Node<'a>> {
    let point = tree_sitter::Point {
        row: position.line,
        column: position.byte,
    };
    let mut node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;

    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn find_named_descendant_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }

    for index in 0..node.child_count() {
        let Some(child) =
            node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };
        if let Some(found) = find_named_descendant_by_kind(child, kind) {
            return Some(found);
        }
    }

    None
}

fn top_level_completions() -> Vec<Completion> {
    vec![
        completion(
            "blueprint",
            Some("Declare blueprint metadata"),
            CompletionKind::Keyword,
        ),
        completion(
            "prompt",
            Some("Declare an interactive prompt"),
            CompletionKind::Keyword,
        ),
    ]
}

fn blueprint_attribute_completions() -> Vec<Completion> {
    vec![
        completion(
            "version",
            Some("Achitekfile schema version"),
            CompletionKind::Property,
        ),
        completion(
            "name",
            Some("Blueprint identifier"),
            CompletionKind::Property,
        ),
        completion(
            "description",
            Some("Blueprint description"),
            CompletionKind::Property,
        ),
        completion("author", Some("Blueprint author"), CompletionKind::Property),
        completion(
            "min_achitek_version",
            Some("Minimum required Achitek version"),
            CompletionKind::Property,
        ),
    ]
}

fn prompt_attribute_completions(syntax: &SourceTree, position: TextPosition) -> Vec<Completion> {
    let prompt_block = ancestor_node_at_position(syntax, position, "prompt_block");
    let prompt_type = prompt_block.and_then(|node| shared::prompt_type_for_block(syntax, node));
    let mut items = vec![
        completion("type", Some("Prompt type"), CompletionKind::Property),
        completion("help", Some("Prompt help text"), CompletionKind::Property),
        completion("default", Some("Default answer"), CompletionKind::Property),
        completion(
            "required",
            Some("Whether the prompt is required"),
            CompletionKind::Property,
        ),
        completion(
            "depends_on",
            Some("Conditional visibility expression"),
            CompletionKind::Property,
        ),
        completion(
            "validate",
            Some("Validation block"),
            CompletionKind::Keyword,
        ),
    ];

    if matches!(prompt_type, None | Some("select" | "multiselect")) {
        items.push(completion(
            "choices",
            Some("Selectable options"),
            CompletionKind::Property,
        ));
    }

    if let Some(prompt_block) = prompt_block {
        items.retain(|item| {
            let kind = match item.label() {
                "type" => "type_attribute",
                "help" => "help_attribute",
                "choices" => "choices_attribute",
                "default" => "default_attribute",
                "required" => "required_attribute",
                "depends_on" => "depends_on_attribute",
                "validate" => "validate_block",
                _ => return true,
            };
            find_named_descendant_by_kind(prompt_block, kind).is_none()
        });
    }

    items
}

fn validate_attribute_completions(syntax: &SourceTree, position: TextPosition) -> Vec<Completion> {
    let prompt_block = ancestor_node_at_position(syntax, position, "prompt_block");
    let validate_block = ancestor_node_at_position(syntax, position, "validate_block");
    let prompt_type = prompt_block.and_then(|node| shared::prompt_type_for_block(syntax, node));
    let mut items = Vec::new();

    if matches!(prompt_type, None | Some("string" | "paragraph")) {
        items.extend([
            completion(
                "regex",
                Some("Regular expression validation"),
                CompletionKind::Property,
            ),
            completion(
                "min_length",
                Some("Minimum string length"),
                CompletionKind::Property,
            ),
            completion(
                "max_length",
                Some("Maximum string length"),
                CompletionKind::Property,
            ),
        ]);
    }

    if matches!(prompt_type, None | Some("multiselect")) {
        items.extend([
            completion(
                "min_selections",
                Some("Minimum number of selected values"),
                CompletionKind::Property,
            ),
            completion(
                "max_selections",
                Some("Maximum number of selected values"),
                CompletionKind::Property,
            ),
        ]);
    }

    if let Some(validate_block) = validate_block {
        items.retain(|item| {
            let kind = match item.label() {
                "regex" => "regex_attribute",
                "min_length" => "min_length_attribute",
                "max_length" => "max_length_attribute",
                "min_selections" => "min_selections_attribute",
                "max_selections" => "max_selections_attribute",
                _ => return true,
            };
            find_named_descendant_by_kind(validate_block, kind).is_none()
        });
    }

    items
}

fn prompt_type_completions() -> Vec<Completion> {
    vec![
        completion(
            "string",
            Some("Single-line text prompt"),
            CompletionKind::Value,
        ),
        completion(
            "paragraph",
            Some("Multi-line text prompt"),
            CompletionKind::Value,
        ),
        completion("bool", Some("Boolean yes/no prompt"), CompletionKind::Value),
        completion(
            "select",
            Some("Single-choice prompt"),
            CompletionKind::Value,
        ),
        completion(
            "multiselect",
            Some("Multi-choice prompt"),
            CompletionKind::Value,
        ),
    ]
}

fn depends_on_completions(symbols: &[Symbol]) -> Vec<Completion> {
    let mut completions = vec![
        completion(
            "all",
            Some("Require all nested conditions"),
            CompletionKind::Function,
        ),
        completion(
            "any",
            Some("Require any nested condition"),
            CompletionKind::Function,
        ),
    ];

    completions.extend(symbols.iter().filter_map(|symbol| {
        if symbol.kind() == SymbolKind::Prompt {
            Some(completion(
                symbol.name(),
                Some("Prompt reference"),
                CompletionKind::Reference,
            ))
        } else {
            None
        }
    }));

    completions
}

fn completion(label: &str, detail: Option<&str>, kind: CompletionKind) -> Completion {
    Completion {
        label: label.to_owned(),
        detail: detail.map(str::to_owned),
        kind,
    }
}
