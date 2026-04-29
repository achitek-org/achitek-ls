//! Analysis
use crate::syntax::{self, ParseError, SyntaxErrorKind, SyntaxTree, TextPosition, TextRange};
use tree_sitter::Node;

/// Analysis result for a single Achitek document.
#[derive(Debug)]
pub struct Analysis {
    syntax: SyntaxTree,
    diagnostics: Vec<Diagnostic>,
    symbols: Vec<Symbol>,
}

impl Analysis {
    /// Returns the parsed syntax tree for the analyzed document.
    pub fn syntax(&self) -> &SyntaxTree {
        &self.syntax
    }

    /// Returns diagnostics produced during analysis.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns true when analysis produced any diagnostics.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Returns document symbols derived from the parsed source.
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Returns hover information for a position in the analyzed document.
    pub fn hover(&self, position: TextPosition) -> Option<Hover> {
        hover_for_position(&self.syntax, position)
    }

    /// Returns completion items for a position in the analyzed document.
    pub fn completions(&self, position: TextPosition) -> Vec<Completion> {
        completions_for_position(&self.syntax, &self.symbols, position)
    }

    /// Returns the definition target for a position in the analyzed document.
    pub fn definition(&self, position: TextPosition) -> Option<DefinitionTarget> {
        definition_for_position(&self.syntax, &self.symbols, position)
    }

    /// Returns rename preparation details for a position in the analyzed document.
    pub fn prepare_rename(&self, position: TextPosition) -> Option<PrepareRenameTarget> {
        prepare_rename_for_position(&self.syntax, &self.symbols, position)
    }

    /// Returns all reference targets related to the symbol under the cursor.
    pub fn references(
        &self,
        position: TextPosition,
        include_declaration: bool,
    ) -> Vec<ReferenceTarget> {
        references_for_position(&self.syntax, &self.symbols, position, include_declaration)
    }

    /// Returns the prompt name associated with the symbol under the cursor.
    pub fn prompt_name(&self, position: TextPosition) -> Option<&str> {
        symbol_name_at_position(&self.syntax, position, &self.symbols)
    }
}

/// A diagnostic that can later be mapped into an LSP diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    severity: Severity,
    message: String,
    range: TextRange,
    related_information: Vec<RelatedInformation>,
}

impl Diagnostic {
    /// Returns the severity of the diagnostic.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the source range associated with the diagnostic.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns related locations that help explain the diagnostic.
    pub fn related_information(&self) -> &[RelatedInformation] {
        &self.related_information
    }
}

/// Diagnostic severity understood by the analysis layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A hard error that should surface prominently in the editor.
    Error,
    /// A warning about suspicious but not necessarily invalid input.
    Warning,
    /// Informational feedback that may help the user understand the document.
    Information,
    /// Low-priority guidance or supporting context for another diagnostic.
    Hint,
}

/// Extra source information attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInformation {
    message: String,
    range: TextRange,
}

impl RelatedInformation {
    /// Returns the related-information message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the related-information source range.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

/// A document symbol derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    range: TextRange,
    selection_range: TextRange,
    children: Vec<Symbol>,
}

/// Hover content derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    contents: String,
    range: TextRange,
}

/// Definition target derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionTarget {
    range: TextRange,
    selection_range: TextRange,
}

/// Prepare-rename target derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareRenameTarget {
    range: TextRange,
    placeholder: String,
}

/// Reference target derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTarget {
    range: TextRange,
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

impl DefinitionTarget {
    /// Returns the full range of the definition target.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the preferred selection range for the definition target.
    pub fn selection_range(&self) -> TextRange {
        self.selection_range
    }
}

impl PrepareRenameTarget {
    /// Returns the source range that should be renamed.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the placeholder name to show before rename.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
}

impl ReferenceTarget {
    /// Returns the source range for the reference target.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

impl Symbol {
    /// Returns the symbol display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns optional symbol detail text.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// Returns the symbol kind.
    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    /// Returns the full source range occupied by the symbol.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the preferred selection range for the symbol.
    pub fn selection_range(&self) -> TextRange {
        self.selection_range
    }

    /// Returns nested child symbols.
    pub fn children(&self) -> &[Symbol] {
        &self.children
    }
}

/// Symbol kinds understood by the analysis layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// The top-level blueprint block.
    Blueprint,
    /// A prompt block.
    Prompt,
    /// A validate block nested inside a prompt.
    Validate,
}

/// Completion kinds understood by the analysis layer.
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

/// Analyzes a single Achitek source document.
///
/// The current implementation parses the source and converts syntax-layer
/// errors into analysis diagnostics. Semantic checks can be added here later
/// without changing the API shape.
pub fn analyze(source: &str) -> Result<Analysis, ParseError> {
    let syntax = syntax::parse(source)?;
    let mut diagnostics: Vec<Diagnostic> = syntax
        .errors()
        .iter()
        .map(|error| Diagnostic {
            severity: Severity::Error,
            message: syntax_error_message(error.kind()).to_owned(),
            range: error.range(),
            related_information: Vec::new(),
        })
        .collect();
    let symbols = collect_symbols(&syntax);
    diagnostics.extend(semantic_diagnostics(&syntax, &symbols));

    Ok(Analysis {
        syntax,
        diagnostics,
        symbols,
    })
}

fn syntax_error_message(kind: SyntaxErrorKind) -> &'static str {
    match kind {
        SyntaxErrorKind::Missing => "missing syntax required to complete this construct",
        SyntaxErrorKind::Unexpected => "unexpected syntax",
    }
}

fn semantic_diagnostics(syntax: &SyntaxTree, symbols: &[Symbol]) -> Vec<Diagnostic> {
    let mut diagnostics = duplicate_prompt_diagnostics(symbols);
    diagnostics.extend(undefined_reference_diagnostics(syntax, symbols));
    diagnostics.extend(prompt_validation_diagnostics(syntax));
    diagnostics
}

fn duplicate_prompt_diagnostics(symbols: &[Symbol]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (index, symbol) in symbols.iter().enumerate() {
        if symbol.kind() != SymbolKind::Prompt {
            continue;
        }

        if let Some(first) = symbols[..index]
            .iter()
            .find(|other| other.kind() == SymbolKind::Prompt && other.name() == symbol.name())
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Hint,
                message: format!("previous definition of prompt `{}` here", symbol.name()),
                range: first.selection_range(),
                related_information: vec![RelatedInformation {
                    message: "duplicate prompt declared here".to_owned(),
                    range: symbol.selection_range(),
                }],
            });
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "duplicate prompt `{}`; first defined at line {}",
                    symbol.name(),
                    first.selection_range().start_position.row + 1
                ),
                range: symbol.selection_range(),
                related_information: vec![RelatedInformation {
                    message: format!("first defined here as `{}`", symbol.name()),
                    range: first.selection_range(),
                }],
            });
        }
    }

    diagnostics
}

fn undefined_reference_diagnostics(syntax: &SyntaxTree, symbols: &[Symbol]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_undefined_reference_diagnostics(syntax.root_node(), syntax, symbols, &mut diagnostics);
    diagnostics
}

fn prompt_validation_diagnostics(syntax: &SyntaxTree) -> Vec<Diagnostic> {
    let root = syntax.root_node();
    let mut diagnostics = Vec::new();

    for index in 0..root.child_count() {
        let Some(child) =
            root.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        if child.kind() == "prompt_block" {
            diagnostics.extend(validate_prompt_block(syntax, child));
        }
    }

    diagnostics
}

fn validate_prompt_block(syntax: &SyntaxTree, prompt_block: Node<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let name_node = prompt_block
        .child_by_field_name("name")
        .expect("prompt_block should have a name field");
    let prompt_name = syntax.text_for(name_node).trim_matches('"');
    let prompt_range = syntax.range_for(name_node);

    let prompt_type = prompt_type_for_block(syntax, prompt_block);
    let choices_attribute = attribute_in_prompt_block(prompt_block, "choices_attribute");
    let default_attribute = attribute_in_prompt_block(prompt_block, "default_attribute");
    let validate_block = child_block(prompt_block, "validate_block");
    let min_length_attribute =
        validate_block.and_then(|block| attribute_in_validate_block(block, "min_length_attribute"));
    let max_length_attribute =
        validate_block.and_then(|block| attribute_in_validate_block(block, "max_length_attribute"));
    let min_selections_attribute = validate_block
        .and_then(|block| attribute_in_validate_block(block, "min_selections_attribute"));
    let max_selections_attribute = validate_block
        .and_then(|block| attribute_in_validate_block(block, "max_selections_attribute"));

    if prompt_type.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: format!("prompt `{prompt_name}` is missing required `type`"),
            range: prompt_range,
            related_information: Vec::new(),
        });
    }

    match prompt_type {
        Some("select") | Some("multiselect") => {
            if choices_attribute.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "prompt `{prompt_name}` of type `{}` requires `choices`",
                        prompt_type.unwrap_or("unknown")
                    ),
                    range: prompt_range,
                    related_information: Vec::new(),
                });
            }
            if let Some(attribute) = choices_attribute
                && choices_for_attribute(syntax, attribute).is_empty()
            {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "`choices` must contain at least one value".to_owned(),
                    range: syntax.range_for(attribute),
                    related_information: Vec::new(),
                });
            }
        }
        Some(other_type) => {
            if let Some(attribute) = choices_attribute {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "`choices` is only valid for `select` and `multiselect` prompts, not `{other_type}`"
                    ),
                    range: syntax.range_for(attribute),
                    related_information: Vec::new(),
                });
            }
        }
        None => {}
    }

    if let (Some(prompt_type), Some(attribute)) = (prompt_type, default_attribute) {
        diagnostics.extend(validate_default_attribute(
            syntax,
            attribute,
            prompt_type,
            choices_attribute,
        ));
    }

    let allows_length_rules = matches!(prompt_type, Some("string" | "paragraph"));
    for attribute in [min_length_attribute, max_length_attribute]
        .into_iter()
        .flatten()
    {
        if !allows_length_rules {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "`{}` is only valid for `string` and `paragraph` prompts",
                    attribute.kind().trim_end_matches("_attribute")
                ),
                range: syntax.range_for(attribute),
                related_information: Vec::new(),
            });
        }
    }

    if let (Some(min), Some(max)) = (
        min_length_attribute.and_then(|attribute| integer_attribute_value(syntax, attribute)),
        max_length_attribute.and_then(|attribute| integer_attribute_value(syntax, attribute)),
    ) && min.value > max.value
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "`min_length` must be less than or equal to `max_length`".to_owned(),
            range: min.range,
            related_information: vec![RelatedInformation {
                message: "`max_length` is declared here".to_owned(),
                range: max.range,
            }],
        });
    }

    let allows_selection_rules = matches!(prompt_type, Some("multiselect"));
    for attribute in [min_selections_attribute, max_selections_attribute]
        .into_iter()
        .flatten()
    {
        if !allows_selection_rules {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!(
                    "`{}` is only valid for `multiselect` prompts",
                    attribute.kind().trim_end_matches("_attribute")
                ),
                range: syntax.range_for(attribute),
                related_information: Vec::new(),
            });
        }
    }

    if let (Some(min), Some(max)) = (
        min_selections_attribute.and_then(|attribute| integer_attribute_value(syntax, attribute)),
        max_selections_attribute.and_then(|attribute| integer_attribute_value(syntax, attribute)),
    ) && min.value > max.value
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: "`min_selections` must be less than or equal to `max_selections`".to_owned(),
            range: min.range,
            related_information: vec![RelatedInformation {
                message: "`max_selections` is declared here".to_owned(),
                range: max.range,
            }],
        });
    }

    diagnostics
}

#[derive(Debug, Clone, Copy)]
struct IntegerValue {
    value: u64,
    range: TextRange,
}

fn validate_default_attribute(
    syntax: &SyntaxTree,
    attribute: Node<'_>,
    prompt_type: &str,
    choices_attribute: Option<Node<'_>>,
) -> Vec<Diagnostic> {
    let Some(value) = attribute.child_by_field_name("value") else {
        return Vec::new();
    };
    let value = unwrap_value_node(value);
    let value_text = syntax.text_for(value);
    let value_range = syntax.range_for(value);
    let mut diagnostics = Vec::new();

    match prompt_type {
        "string" | "paragraph" => {
            if value.kind() != "string_literal" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("default for `{prompt_type}` prompts must be a string"),
                    range: value_range,
                    related_information: Vec::new(),
                });
            }
        }
        "bool" => {
            if value.kind() != "boolean" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "default for `bool` prompts must be `true` or `false`".to_owned(),
                    range: value_range,
                    related_information: Vec::new(),
                });
            }
        }
        "select" => {
            if value.kind() != "string_literal" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "default for `select` prompts must be one choice string".to_owned(),
                    range: value_range,
                    related_information: Vec::new(),
                });
            } else if let Some(choices_attribute) = choices_attribute {
                let choices = choices_for_attribute(syntax, choices_attribute);
                let default = unquote(value_text);
                if !choices.iter().any(|choice| choice == default) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("default `{default}` is not listed in `choices`"),
                        range: value_range,
                        related_information: Vec::new(),
                    });
                }
            }
        }
        "multiselect" => {
            if value.kind() != "array" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "default for `multiselect` prompts must be an array".to_owned(),
                    range: value_range,
                    related_information: Vec::new(),
                });
            } else if let Some(choices_attribute) = choices_attribute {
                let choices = choices_for_attribute(syntax, choices_attribute);
                for item in string_values_in_array(syntax, value) {
                    if !choices.iter().any(|choice| choice == &item.value) {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            message: format!(
                                "default choice `{}` is not listed in `choices`",
                                item.value
                            ),
                            range: item.range,
                            related_information: Vec::new(),
                        });
                    }
                }
            }
        }
        _ => {}
    }

    diagnostics
}

#[derive(Debug, Clone)]
struct StringValue {
    value: String,
    range: TextRange,
}

fn choices_for_attribute(syntax: &SyntaxTree, attribute: Node<'_>) -> Vec<String> {
    let Some(value) = attribute.child_by_field_name("value") else {
        return Vec::new();
    };
    string_values_in_array(syntax, unwrap_value_node(value))
        .into_iter()
        .map(|value| value.value)
        .collect()
}

fn unwrap_value_node(mut node: Node<'_>) -> Node<'_> {
    while matches!(node.kind(), "value" | "literal_value") {
        let Some(child) = first_named_child(node) else {
            break;
        };
        node = child;
    }
    node
}

fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    for index in 0..node.named_child_count() {
        let Some(child) =
            node.named_child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };
        return Some(child);
    }
    None
}

fn string_values_in_array(syntax: &SyntaxTree, array: Node<'_>) -> Vec<StringValue> {
    let mut values = Vec::new();
    collect_string_values(array, syntax, &mut values);
    values
}

fn collect_string_values(node: Node<'_>, syntax: &SyntaxTree, values: &mut Vec<StringValue>) {
    if node.kind() == "string_literal" {
        values.push(StringValue {
            value: unquote(syntax.text_for(node)).to_owned(),
            range: syntax.range_for(node),
        });
    }

    for index in 0..node.child_count() {
        let Some(child) =
            node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };
        collect_string_values(child, syntax, values);
    }
}

fn integer_attribute_value(syntax: &SyntaxTree, attribute: Node<'_>) -> Option<IntegerValue> {
    let value = attribute.child_by_field_name("value")?;
    Some(IntegerValue {
        value: syntax.text_for(value).parse().ok()?,
        range: syntax.range_for(value),
    })
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn child_block<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    for index in 0..node.child_count() {
        let Some(child) =
            node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

fn attribute_in_prompt_block<'a>(prompt_block: Node<'a>, kind: &str) -> Option<Node<'a>> {
    find_named_descendant_by_kind(prompt_block, kind)
}

fn attribute_in_validate_block<'a>(validate_block: Node<'a>, kind: &str) -> Option<Node<'a>> {
    find_named_descendant_by_kind(validate_block, kind)
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

fn collect_undefined_reference_diagnostics(
    node: Node<'_>,
    syntax: &SyntaxTree,
    symbols: &[Symbol],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if node.kind() == "identifier"
        && let Some(name) = identifier_reference_name(syntax, node)
        && !symbols
            .iter()
            .any(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == name)
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            message: format!("undefined prompt reference `{name}`"),
            range: syntax.range_for(node),
            related_information: Vec::new(),
        });
    }

    for index in 0..node.child_count() {
        let Some(child) =
            node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };
        collect_undefined_reference_diagnostics(child, syntax, symbols, diagnostics);
    }
}

fn definition_for_position(
    syntax: &SyntaxTree,
    symbols: &[Symbol],
    position: TextPosition,
) -> Option<DefinitionTarget> {
    let point = tree_sitter::Point {
        row: position.row,
        column: position.column,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;
    let reference_name = match node.kind() {
        "identifier" => identifier_reference_name(syntax, node),
        _ => None,
    }?;

    let symbol = symbols
        .iter()
        .find(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == reference_name)?;

    Some(DefinitionTarget {
        range: symbol.range(),
        selection_range: symbol.selection_range(),
    })
}

fn prepare_rename_for_position(
    syntax: &SyntaxTree,
    symbols: &[Symbol],
    position: TextPosition,
) -> Option<PrepareRenameTarget> {
    let point = tree_sitter::Point {
        row: position.row,
        column: position.column,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;

    match node.kind() {
        "identifier" => {
            let name = identifier_reference_name(syntax, node)?;
            Some(PrepareRenameTarget {
                range: syntax.range_for(node),
                placeholder: name.to_owned(),
            })
        }
        "prompt_block" => {
            let name_node = node.child_by_field_name("name")?;
            let name = syntax.text_for(name_node).trim_matches('"');
            symbols
                .iter()
                .find(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == name)?;
            Some(PrepareRenameTarget {
                range: syntax.range_for(name_node),
                placeholder: name.to_owned(),
            })
        }
        "string_literal" => {
            let parent = node.parent()?;
            if parent.kind() == "prompt_block" && parent.child_by_field_name("name") == Some(node) {
                let name = syntax.text_for(node).trim_matches('"');
                symbols
                    .iter()
                    .find(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == name)?;
                Some(PrepareRenameTarget {
                    range: syntax.range_for(node),
                    placeholder: name.to_owned(),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn references_for_position(
    syntax: &SyntaxTree,
    symbols: &[Symbol],
    position: TextPosition,
    include_declaration: bool,
) -> Vec<ReferenceTarget> {
    let Some(name) = symbol_name_at_position(syntax, position, symbols) else {
        return Vec::new();
    };

    let mut references = Vec::new();

    if include_declaration
        && let Some(symbol) = symbols
            .iter()
            .find(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == name)
    {
        references.push(ReferenceTarget {
            range: symbol.selection_range(),
        });
    }

    collect_reference_nodes(syntax.root_node(), syntax, name, &mut references);
    references
}

fn symbol_name_at_position<'a>(
    syntax: &'a SyntaxTree,
    position: TextPosition,
    symbols: &[Symbol],
) -> Option<&'a str> {
    let point = tree_sitter::Point {
        row: position.row,
        column: position.column,
    };
    let node = syntax
        .root_node()
        .named_descendant_for_point_range(point, point)?;

    match node.kind() {
        "identifier" => identifier_reference_name(syntax, node),
        "prompt_block" => {
            let name_node = node.child_by_field_name("name")?;
            let name = syntax.text_for(name_node).trim_matches('"');
            symbols
                .iter()
                .find(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == name)?;
            Some(name)
        }
        "string_literal" => {
            let parent = node.parent()?;
            if parent.kind() == "prompt_block" && parent.child_by_field_name("name") == Some(node) {
                let name = syntax.text_for(node).trim_matches('"');
                symbols
                    .iter()
                    .find(|symbol| symbol.kind() == SymbolKind::Prompt && symbol.name() == name)?;
                Some(name)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn collect_reference_nodes(
    node: Node<'_>,
    syntax: &SyntaxTree,
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

fn identifier_reference_name<'a>(syntax: &'a SyntaxTree, node: Node<'_>) -> Option<&'a str> {
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

fn completions_for_position(
    syntax: &SyntaxTree,
    symbols: &[Symbol],
    position: TextPosition,
) -> Vec<Completion> {
    let line = source_line(syntax.source(), position.row);
    let prefix = prefix_before_column(line, position.column);
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

fn in_prompt_block(syntax: &SyntaxTree, position: TextPosition) -> bool {
    ancestor_kinds_at_position(syntax, position).contains(&"prompt_block")
}

fn in_validate_block(syntax: &SyntaxTree, position: TextPosition) -> bool {
    ancestor_kinds_at_position(syntax, position).contains(&"validate_block")
}

fn in_blueprint_block(syntax: &SyntaxTree, position: TextPosition) -> bool {
    ancestor_kinds_at_position(syntax, position).contains(&"blueprint_block")
}

fn ancestor_kinds_at_position(syntax: &SyntaxTree, position: TextPosition) -> Vec<&str> {
    let point = tree_sitter::Point {
        row: position.row,
        column: position.column,
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
    syntax: &'a SyntaxTree,
    position: TextPosition,
    kind: &str,
) -> Option<Node<'a>> {
    let point = tree_sitter::Point {
        row: position.row,
        column: position.column,
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

fn prompt_attribute_completions(syntax: &SyntaxTree, position: TextPosition) -> Vec<Completion> {
    let prompt_block = ancestor_node_at_position(syntax, position, "prompt_block");
    let prompt_type = prompt_block.and_then(|node| prompt_type_for_block(syntax, node));
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

fn validate_attribute_completions(syntax: &SyntaxTree, position: TextPosition) -> Vec<Completion> {
    let prompt_block = ancestor_node_at_position(syntax, position, "prompt_block");
    let validate_block = ancestor_node_at_position(syntax, position, "validate_block");
    let prompt_type = prompt_block.and_then(|node| prompt_type_for_block(syntax, node));
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

fn hover_for_position(syntax: &SyntaxTree, position: TextPosition) -> Option<Hover> {
    let point = tree_sitter::Point {
        row: position.row,
        column: position.column,
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

fn hover_for_prompt_block(syntax: &SyntaxTree, node: Node<'_>) -> Option<Hover> {
    let name_node = node.child_by_field_name("name")?;
    let name = syntax.text_for(name_node).trim_matches('"');
    let prompt_type = prompt_type_for_block(syntax, node).unwrap_or("unknown");

    Some(simple_hover(
        syntax.range_for(name_node),
        format!(
            "## prompt `{name}`\n\nType: `{prompt_type}`\n\nDefines an interactive prompt in the Achitekfile."
        ),
    ))
}

fn hover_for_blueprint_attribute_key(syntax: &SyntaxTree, node: Node<'_>) -> Option<Hover> {
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

fn hover_for_prompt_type(syntax: &SyntaxTree, node: Node<'_>) -> Option<Hover> {
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

fn prompt_type_for_block<'a>(syntax: &'a SyntaxTree, prompt_block: Node<'_>) -> Option<&'a str> {
    for index in 0..prompt_block.child_count() {
        let Some(child) =
            prompt_block.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        if child.kind() == "question_attribute" {
            for nested_index in 0..child.child_count() {
                let Some(nested) = child
                    .child(u32::try_from(nested_index).expect("child index should fit into u32"))
                else {
                    continue;
                };

                if nested.kind() == "type_attribute" {
                    let value = nested.child_by_field_name("value")?;
                    return Some(syntax.text_for(value));
                }
            }
        }
    }

    None
}

fn simple_hover(range: TextRange, contents: impl Into<String>) -> Hover {
    Hover {
        contents: contents.into(),
        range,
    }
}

fn collect_symbols(syntax: &SyntaxTree) -> Vec<Symbol> {
    let root = syntax.root_node();
    let mut symbols = Vec::new();

    for index in 0..root.child_count() {
        let Some(child) =
            root.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        match child.kind() {
            "blueprint_block" => symbols.push(blueprint_symbol(syntax, child)),
            "prompt_block" => symbols.push(prompt_symbol(syntax, child)),
            _ => {}
        }
    }

    symbols
}

fn blueprint_symbol(syntax: &SyntaxTree, node: Node<'_>) -> Symbol {
    let range = syntax.range_for(node);

    Symbol {
        name: "blueprint".to_owned(),
        detail: None,
        kind: SymbolKind::Blueprint,
        range,
        selection_range: range,
        children: Vec::new(),
    }
}

fn prompt_symbol(syntax: &SyntaxTree, node: Node<'_>) -> Symbol {
    let range = syntax.range_for(node);
    let name_node = node
        .child_by_field_name("name")
        .expect("prompt_block should have a name field");
    let selection_range = syntax.range_for(name_node);
    let name = syntax.text_for(name_node).trim_matches('"').to_owned();
    let children = collect_prompt_children(syntax, node);

    Symbol {
        name,
        detail: Some("prompt".to_owned()),
        kind: SymbolKind::Prompt,
        range,
        selection_range,
        children,
    }
}

fn collect_prompt_children(syntax: &SyntaxTree, prompt_node: Node<'_>) -> Vec<Symbol> {
    let mut children = Vec::new();

    for index in 0..prompt_node.child_count() {
        let Some(child) =
            prompt_node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        if child.kind() == "validate_block" {
            let range = syntax.range_for(child);
            children.push(Symbol {
                name: "validate".to_owned(),
                detail: None,
                kind: SymbolKind::Validate,
                range,
                selection_range: range,
                children: Vec::new(),
            });
        }
    }

    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn analyzes_valid_source_without_diagnostics() {
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

        let analysis = analyze(source).expect("valid source should analyze");

        assert_eq!(analysis.syntax().root_node().kind(), "file");
        assert!(!analysis.has_diagnostics());
        assert!(analysis.diagnostics().is_empty());
    }

    #[test]
    fn surfaces_syntax_diagnostics_for_invalid_source() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "broken"

            prompt "project_name" {
              type = string
            }
        "#};

        let analysis = analyze(source).expect("invalid source should still analyze");

        assert!(analysis.has_diagnostics());
        assert!(
            analysis
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.severity() == Severity::Error)
        );
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| !diagnostic.message().is_empty())
        );
    }

    #[test]
    fn reports_duplicate_prompt_names() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }

            prompt "project_name" {
              type = string
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let diagnostics = analysis.diagnostics();

        let duplicate = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message().contains("duplicate prompt"))
            .expect("expected duplicate diagnostic");
        let original = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .message()
                    .contains("previous definition of prompt")
            })
            .expect("expected original-location diagnostic");
        assert_eq!(
            duplicate.message(),
            "duplicate prompt `project_name`; first defined at line 6"
        );
        assert_eq!(
            original.message(),
            "previous definition of prompt `project_name` here"
        );
        assert_eq!(original.severity(), Severity::Hint);
        assert_eq!(original.range().start_position.row, 5);
        assert_eq!(original.related_information().len(), 1);
        assert_eq!(
            original.related_information()[0].message(),
            "duplicate prompt declared here"
        );
        assert_eq!(
            original.related_information()[0].range().start_position.row,
            9
        );
        assert_eq!(duplicate.related_information().len(), 1);
        assert_eq!(
            duplicate.related_information()[0].message(),
            "first defined here as `project_name`"
        );
        assert_eq!(
            duplicate.related_information()[0]
                .range()
                .start_position
                .row,
            5
        );
    }

    #[test]
    fn reports_undefined_prompt_references() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = string
              depends_on = missing_prompt
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let diagnostics = analysis.diagnostics();

        assert!(diagnostics.iter().any(
            |diagnostic| diagnostic.message() == "undefined prompt reference `missing_prompt`"
        ));
    }

    #[test]
    fn reports_missing_prompt_type() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              help = "Project name"
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "prompt `project_name` is missing required `type`")
        );
    }

    #[test]
    fn reports_missing_choices_for_select_prompt() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = select
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "prompt `kind` of type `select` requires `choices`")
        );
    }

    #[test]
    fn reports_choices_on_non_select_prompt() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              choices = ["a", "b"]
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(analysis.diagnostics().iter().any(
            |diagnostic| diagnostic.message()
                == "`choices` is only valid for `select` and `multiselect` prompts, not `string`"
        ));
    }

    #[test]
    fn reports_string_length_rules_on_non_string_prompt() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "enabled" {
              type = bool

              validate {
                min_length = 2
              }
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "`min_length` is only valid for `string` and `paragraph` prompts")
        );
    }

    #[test]
    fn reports_selection_rules_on_non_multiselect_prompt() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = select
              choices = ["a", "b"]

              validate {
                min_selections = 1
              }
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "`min_selections` is only valid for `multiselect` prompts")
        );
    }

    #[test]
    fn reports_default_value_type_mismatches() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "enabled" {
              type = bool
              default = "yes"
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "default for `bool` prompts must be `true` or `false`")
        );
    }

    #[test]
    fn reports_select_default_not_in_choices() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = select
              choices = ["bin", "lib"]
              default = "cli"
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis.diagnostics().iter().any(
                |diagnostic| diagnostic.message() == "default `cli` is not listed in `choices`"
            )
        );
    }

    #[test]
    fn reports_multiselect_default_not_in_choices() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "features" {
              type = multiselect
              choices = ["serde", "tokio"]
              default = ["serde", "tracing"]
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "default choice `tracing` is not listed in `choices`")
        );
    }

    #[test]
    fn reports_empty_choices() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = select
              choices = []
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "`choices` must contain at least one value")
        );
    }

    #[test]
    fn reports_invalid_validation_ranges() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "name" {
              type = string

              validate {
                min_length = 10
                max_length = 2
              }
            }

            prompt "features" {
              type = multiselect
              choices = ["serde", "tokio"]

              validate {
                min_selections = 3
                max_selections = 1
              }
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "`min_length` must be less than or equal to `max_length`")
        );
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message()
                    == "`min_selections` must be less than or equal to `max_selections`")
        );
    }

    #[test]
    fn filters_prompt_attribute_completions_by_type_and_existing_attributes() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "name" {
              type = string

            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let completions = analysis.completions(TextPosition { row: 7, column: 2 });

        assert!(!completions.iter().any(|item| item.label() == "type"));
        assert!(!completions.iter().any(|item| item.label() == "choices"));
        assert!(completions.iter().any(|item| item.label() == "default"));
    }

    #[test]
    fn filters_validate_completions_by_prompt_type() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = multiselect
              choices = ["a", "b"]

              validate {

              }
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let completions = analysis.completions(TextPosition { row: 10, column: 4 });

        assert!(
            completions
                .iter()
                .any(|item| item.label() == "min_selections")
        );
        assert!(!completions.iter().any(|item| item.label() == "min_length"));
    }

    #[test]
    fn collects_document_symbols() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"

              validate {
                min_length = 2
              }
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");

        assert_eq!(analysis.symbols().len(), 2);
        assert_eq!(analysis.symbols()[0].name(), "blueprint");
        assert_eq!(analysis.symbols()[0].kind(), SymbolKind::Blueprint);
        assert_eq!(analysis.symbols()[1].name(), "project_name");
        assert_eq!(analysis.symbols()[1].kind(), SymbolKind::Prompt);
        assert_eq!(analysis.symbols()[1].children().len(), 1);
        assert_eq!(analysis.symbols()[1].children()[0].name(), "validate");
        assert_eq!(
            analysis.symbols()[1].children()[0].kind(),
            SymbolKind::Validate
        );
    }

    #[test]
    fn returns_hover_for_prompt_type() {
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

        let analysis = analyze(source).expect("valid source should analyze");
        let hover = analysis
            .hover(TextPosition { row: 6, column: 9 })
            .expect("hover should exist for prompt type");

        assert!(hover.contents().contains("`string`"));
        assert!(hover.contents().contains("single-line text prompt"));
    }

    #[test]
    fn returns_prompt_type_completions() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = 
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let completions = analysis.completions(TextPosition { row: 6, column: 9 });

        assert!(completions.iter().any(|item| item.label() == "string"));
        assert!(completions.iter().any(|item| item.label() == "paragraph"));
    }

    #[test]
    fn returns_depends_on_completions() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = 
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let completions = analysis.completions(TextPosition {
            row: 13,
            column: 15,
        });

        assert!(completions.iter().any(|item| item.label() == "all"));
        assert!(
            completions
                .iter()
                .any(|item| item.label() == "project_name")
        );
    }

    #[test]
    fn resolves_definition_for_prompt_reference() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = project_name
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let definition = analysis
            .definition(TextPosition {
                row: 13,
                column: 16,
            })
            .expect("definition should exist for prompt reference");

        assert_eq!(definition.selection_range().start_position.row, 5);
        assert_eq!(definition.selection_range().start_position.column, 7);
    }

    #[test]
    fn finds_references_for_prompt_definition() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = project_name
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let references = analysis.references(TextPosition { row: 5, column: 9 }, true);

        assert_eq!(references.len(), 2);
        assert_eq!(references[0].range().start_position.row, 5);
        assert_eq!(references[1].range().start_position.row, 13);
    }

    #[test]
    fn prepares_rename_for_prompt_definition() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let target = analysis
            .prepare_rename(TextPosition { row: 5, column: 10 })
            .expect("prepare rename should exist for prompt definition");

        assert_eq!(target.placeholder(), "project_name");
        assert_eq!(target.range().start_position.row, 5);
        assert_eq!(target.range().start_position.column, 7);
    }

    #[test]
    fn prepares_rename_for_prompt_reference() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }

            prompt "kind" {
              type = string
              depends_on = project_name
            }
        "#};

        let analysis = analyze(source).expect("valid source should analyze");
        let target = analysis
            .prepare_rename(TextPosition {
                row: 11,
                column: 16,
            })
            .expect("prepare rename should exist for prompt reference");

        assert_eq!(target.placeholder(), "project_name");
        assert_eq!(target.range().start_position.row, 11);
        assert_eq!(target.range().start_position.column, 15);
    }
}
