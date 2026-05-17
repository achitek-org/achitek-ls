use super::syntax::{named_children, text, text_range_for_node};
use crate::{
    Diagnostic, DiagnosticCode, TextRange,
    model::{AchitekFile, Dependency, Prompt, PromptType, Spanned, Value},
    sort::{Graph, analyze_graph},
};
use std::collections::{HashMap, HashSet};
use tree_sitter::{Node, Tree};

pub(super) fn collect_diagnostics(
    tree: &Tree,
    source: &str,
    file: &AchitekFile,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_file_shape_diagnostics(tree.root_node(), source, &mut diagnostics);
    collect_syntax_diagnostics(tree.root_node(), source, &mut diagnostics);
    collect_source_syntax_diagnostics(tree.root_node(), source, &mut diagnostics);
    collect_tree_semantic_diagnostics(tree.root_node(), source, &mut diagnostics);
    collect_semantic_diagnostics(file, &mut diagnostics);
    collect_dependency_diagnostics(file, &mut diagnostics);
    collect_validation_diagnostics(file, &mut diagnostics);
    diagnostics
}

fn collect_file_shape_diagnostics(root: Node<'_>, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = root.walk();
    let mut blueprint_count = 0;
    let mut saw_blueprint = false;
    let mut reported_prompt_before_blueprint = false;

    for node in root.named_children(&mut cursor) {
        match node.kind() {
            "blueprint_block" => {
                blueprint_count += 1;
                saw_blueprint = true;

                if blueprint_count > 1 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::MultipleBlueprintBlocks,
                        text_range_for_node(node),
                    ));
                }
            }
            "prompt_block" if !saw_blueprint && !reported_prompt_before_blueprint => {
                reported_prompt_before_blueprint = true;
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PromptBeforeBlueprint,
                    text_range_for_node(node),
                ));
            }
            "ERROR" if starts_with_keyword(text(node, source), "blueprint") => {
                blueprint_count += 1;
                saw_blueprint = true;

                if blueprint_count > 1 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::MultipleBlueprintBlocks,
                        text_range_for_node(node),
                    ));
                }
            }
            "ERROR"
                if starts_with_keyword(text(node, source), "prompt")
                    && !saw_blueprint
                    && !reported_prompt_before_blueprint =>
            {
                reported_prompt_before_blueprint = true;
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PromptBeforeBlueprint,
                    text_range_for_node(node),
                ));
            }
            _ => {}
        }
    }

    if blueprint_count == 0 {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingBlueprintBlock,
            text_range_for_node(root),
        ));
    }
}

fn collect_source_syntax_diagnostics(
    root: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root_range = text_range_for_node(root);

    if source.lines().any(|line| {
        let line = line.trim();
        let Some(value) = line
            .strip_prefix("type")
            .and_then(|line| line.trim_start().strip_prefix('='))
        else {
            return false;
        };
        !matches!(
            value.trim(),
            "string" | "paragraph" | "bool" | "select" | "multiselect"
        )
    }) {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnknownPromptType,
            root_range,
        ));
    }

    if source.lines().any(|line| {
        let line = line.trim();
        let Some(value) = line
            .strip_prefix("required")
            .and_then(|line| line.trim_start().strip_prefix('='))
        else {
            return false;
        };
        !matches!(value.trim(), "true" | "false")
    }) {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::InvalidBooleanLiteral,
            root_range,
        ));
    }

    if source.lines().any(|line| {
        let line = line.trim();
        [
            "min_length",
            "max_length",
            "min_selections",
            "max_selections",
        ]
        .iter()
        .any(|name| {
            line.strip_prefix(name)
                .and_then(|line| line.trim_start().strip_prefix('='))
                .is_some_and(|value| value.trim().starts_with('-'))
        })
    }) {
        diagnostics.push(Diagnostic::new(DiagnosticCode::InvalidInteger, root_range));
    }

    if source.lines().any(|line| line.trim_end().ends_with('=')) {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MissingAttributeValue,
            root_range,
        ));
    }
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_syntax_diagnostics(child, source, diagnostics);
    }
}

fn collect_tree_semantic_diagnostics(
    root: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "blueprint_block" => collect_blueprint_tree_diagnostics(child, source, diagnostics),
            "prompt_block" => collect_prompt_tree_diagnostics(child, source, diagnostics),
            _ => {}
        }
    }
}

fn collect_blueprint_tree_diagnostics(
    node: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();

    for child in named_children(node).filter(|node| node.kind() == "blueprint_attribute") {
        let Some(key_node) = child.child_by_field_name("key") else {
            continue;
        };
        if !seen.insert(text(key_node, source)) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateBlueprintAttribute,
                text_range_for_node(child),
            ));
        }
    }
}

fn collect_prompt_tree_diagnostics(
    node: Node<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_attributes = HashSet::new();
    let mut seen_validate_blocks = 0;

    for child in named_children(node) {
        match child.kind() {
            "question_attribute" => {
                let Some(attribute) = named_children(child).next() else {
                    continue;
                };
                if !seen_attributes.insert(attribute.kind()) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DuplicatePromptAttribute,
                        text_range_for_node(attribute),
                    ));
                }
            }
            "validate_block" => {
                seen_validate_blocks += 1;
                if seen_validate_blocks > 1 {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DuplicateValidateBlock,
                        text_range_for_node(child),
                    ));
                }
                collect_validate_tree_diagnostics(child, diagnostics);
            }
            _ => {
                let _ = source;
            }
        }
    }
}

fn collect_validate_tree_diagnostics(node: Node<'_>, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen_attributes = HashSet::new();

    for child in named_children(node).filter(|node| node.kind() == "validate_attribute") {
        let Some(attribute) = named_children(child).next() else {
            continue;
        };
        if !seen_attributes.insert(attribute.kind()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicateValidateAttribute,
                text_range_for_node(attribute),
            ));
        }
    }
}

fn collect_semantic_diagnostics(file: &AchitekFile, diagnostics: &mut Vec<Diagnostic>) {
    let blueprint = file.blueprint();

    if let Some(blueprint_range) = blueprint.range {
        match &blueprint.version {
            Some(version) if version.value.is_empty() => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::EmptyBlueprintVersion,
                    version.range,
                ));
            }
            Some(_) => {}
            None => {
                diagnostics.push(Diagnostic::with_message(
                    DiagnosticCode::MissingBlueprintVersion,
                    blueprint_range,
                    "missing required blueprint `version` attribute",
                ));
            }
        }

        match &blueprint.name {
            Some(name) if name.value.is_empty() => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::EmptyBlueprintName,
                    name.range,
                ));
            }
            Some(_) => {}
            None => {
                diagnostics.push(Diagnostic::with_message(
                    DiagnosticCode::MissingBlueprintName,
                    blueprint_range,
                    "missing required blueprint `name` attribute",
                ));
            }
        }

        if let Some(version) = &blueprint.version
            && !version.value.is_empty()
            && !is_three_component_numeric_version(&version.value)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidBlueprintVersion,
                version.range,
            ));
        }

        if let Some(version) = &blueprint.min_achitek_version
            && !version.value.is_empty()
            && !is_three_component_numeric_version(&version.value)
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidMinimumAchitekVersion,
                version.range,
            ));
        }
    }

    let mut seen_prompt_names = HashSet::new();
    for prompt in file.prompts() {
        let prompt_range = prompt.range;
        let prompt = &prompt.value;

        if prompt.name.is_empty() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::EmptyPromptName,
                prompt_range,
            ));
        } else if !seen_prompt_names.insert(prompt.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::DuplicatePromptName,
                prompt_range,
            ));
        }

        let Some(prompt_type) = prompt.prompt_type else {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingPromptType,
                prompt_range,
            ));
            continue;
        };

        collect_choice_diagnostics(prompt, prompt_type, prompt_range, diagnostics);
        collect_default_diagnostics(prompt, prompt_type, prompt_range, diagnostics);

        if prompt.required == Some(false) && prompt.default.is_none() {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::RequiredFalseWithNoDefault,
                prompt_range,
            ));
        }
    }
}

fn collect_choice_diagnostics(
    prompt: &Prompt,
    prompt_type: PromptType,
    range: TextRange,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if prompt.choices_declared && prompt.choices.is_empty() {
        match prompt_type {
            PromptType::Select | PromptType::MultiSelect => {
                diagnostics.push(Diagnostic::new(DiagnosticCode::EmptyChoicesList, range));
            }
            PromptType::String | PromptType::Paragraph | PromptType::Bool => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::ChoicesOnNonChoicePrompt,
                    range,
                ));
            }
        }
        return;
    }

    match prompt_type {
        PromptType::Select if prompt.choices.is_empty() => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingChoicesForSelect,
                range,
            ));
        }
        PromptType::MultiSelect if prompt.choices.is_empty() => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::MissingChoicesForMultiselect,
                range,
            ));
        }
        PromptType::String | PromptType::Paragraph | PromptType::Bool
            if prompt.choices_declared =>
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ChoicesOnNonChoicePrompt,
                range,
            ));
            return;
        }
        _ => {}
    }

    if matches!(
        prompt_type,
        PromptType::String | PromptType::Paragraph | PromptType::Bool
    ) {
        return;
    }

    let mut seen_choices = HashSet::new();
    let mut all_choices_were_strings = true;
    for choice in &prompt.choices {
        let Value::String(choice) = choice else {
            all_choices_were_strings = false;
            continue;
        };

        if !seen_choices.insert(choice.as_str()) {
            diagnostics.push(Diagnostic::new(DiagnosticCode::DuplicateChoice, range));
        }
    }

    if !all_choices_were_strings {
        diagnostics.push(Diagnostic::new(DiagnosticCode::NonStringChoice, range));
    }
}

fn collect_default_diagnostics(
    prompt: &Prompt,
    prompt_type: PromptType,
    range: TextRange,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(default) = &prompt.default else {
        return;
    };

    if prompt_type == PromptType::MultiSelect && !matches!(default, Value::Array(_)) {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::MultiselectDefaultMustBeArray,
            range,
        ));
        return;
    }

    if !default_matches_prompt_type(default, prompt_type) {
        diagnostics.push(Diagnostic::new(DiagnosticCode::DefaultTypeMismatch, range));
        return;
    }

    match (prompt_type, default) {
        (PromptType::Select, Value::String(default))
            if !string_choices(prompt).contains(default) =>
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SelectDefaultNotInChoices,
                range,
            ));
        }
        (PromptType::MultiSelect, Value::Array(defaults)) => {
            let choices = string_choices(prompt);
            if defaults.iter().any(|value| match value {
                Value::String(value) => !choices.contains(value),
                _ => true,
            }) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::MultiselectDefaultContainsUnknownChoice,
                    range,
                ));
            }
        }
        _ => {}
    }
}

fn collect_dependency_diagnostics(file: &AchitekFile, diagnostics: &mut Vec<Diagnostic>) {
    let prompts_by_name = file
        .prompts()
        .iter()
        .map(|prompt| (prompt.value.name.as_str(), prompt))
        .collect::<HashMap<_, _>>();

    for prompt in file.prompts() {
        let Some(dependency) = &prompt.value.depends_on else {
            continue;
        };

        collect_dependency_expr_diagnostics(
            &prompt.value.name,
            prompt.range,
            dependency,
            &prompts_by_name,
            diagnostics,
        );
    }

    let graph = dependency_graph(file);
    let graph_analysis = analyze_graph(&graph);
    for cycle in graph_analysis.cycles {
        for node in cycle.nodes {
            let range = prompts_by_name
                .get(node.as_str())
                .map(|prompt| prompt.range)
                .unwrap_or_default();
            diagnostics.push(Diagnostic::new(DiagnosticCode::DependencyCycle, range));
        }
    }
}

fn collect_dependency_expr_diagnostics(
    owner: &str,
    range: TextRange,
    dependency: &Dependency,
    prompts_by_name: &HashMap<&str, &Spanned<Prompt>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match dependency {
        Dependency::Reference(reference) => {
            collect_dependency_reference_diagnostics(
                owner,
                reference,
                range,
                prompts_by_name,
                diagnostics,
            );
        }
        Dependency::Comparison { left, right, .. } => {
            collect_dependency_reference_diagnostics(
                owner,
                left,
                range,
                prompts_by_name,
                diagnostics,
            );
            if let Some(prompt) = prompts_by_name.get(left.as_str())
                && let Some(prompt_type) = prompt.value.prompt_type
                && !literal_matches_dependency_type(right, prompt_type)
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::DependencyTypeMismatch,
                    range,
                ));
            }
        }
        Dependency::Contains { receiver, argument } => {
            collect_dependency_reference_diagnostics(
                owner,
                receiver,
                range,
                prompts_by_name,
                diagnostics,
            );
            if let Some(prompt) = prompts_by_name.get(receiver.as_str()) {
                if prompt.value.prompt_type != Some(PromptType::MultiSelect) {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::ContainsOnNonMultiselectPrompt,
                        range,
                    ));
                } else if let Value::String(value) = argument {
                    if !string_choices(&prompt.value).contains(value) {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::ContainsUnknownChoice,
                            range,
                        ));
                    }
                } else {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::DependencyTypeMismatch,
                        range,
                    ));
                }
            }
        }
        Dependency::All(dependencies) | Dependency::Any(dependencies) => {
            for dependency in dependencies {
                collect_dependency_expr_diagnostics(
                    owner,
                    range,
                    dependency,
                    prompts_by_name,
                    diagnostics,
                );
            }
        }
    }
}

fn collect_dependency_reference_diagnostics(
    owner: &str,
    reference: &str,
    range: TextRange,
    prompts_by_name: &HashMap<&str, &Spanned<Prompt>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reference == owner {
        diagnostics.push(Diagnostic::new(DiagnosticCode::SelfDependency, range));
    }

    if !prompts_by_name.contains_key(reference) {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::UnknownDependencyReference,
            range,
        ));
    }
}

fn collect_validation_diagnostics(file: &AchitekFile, diagnostics: &mut Vec<Diagnostic>) {
    for prompt in file.prompts() {
        let prompt_type = prompt.value.prompt_type;
        let validation = &prompt.value.validation;
        let range = prompt.range;

        let has_string_validation = validation.regex.is_some()
            || validation.min_length.is_some()
            || validation.max_length.is_some();
        if has_string_validation
            && !matches!(
                prompt_type,
                Some(PromptType::String) | Some(PromptType::Paragraph)
            )
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::StringValidationOnNonStringPrompt,
                range,
            ));
        }

        let has_selection_validation =
            validation.min_selections.is_some() || validation.max_selections.is_some();
        if has_selection_validation && prompt_type != Some(PromptType::MultiSelect) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SelectionValidationOnNonMultiselectPrompt,
                range,
            ));
        }

        if let (Some(min), Some(max)) = (validation.min_length, validation.max_length)
            && min > max
        {
            diagnostics.push(Diagnostic::new(DiagnosticCode::InvalidLengthBounds, range));
        }

        if let (Some(min), Some(max)) = (validation.min_selections, validation.max_selections)
            && min > max
        {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidSelectionBounds,
                range,
            ));
        }

        if let Some(regex) = &validation.regex
            && regex::Regex::new(regex).is_err()
        {
            diagnostics.push(Diagnostic::new(DiagnosticCode::InvalidRegex, range));
        }
    }
}

fn is_three_component_numeric_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };

    parts.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && !patch.is_empty()
        && major.chars().all(|ch| ch.is_ascii_digit())
        && minor.chars().all(|ch| ch.is_ascii_digit())
        && patch.chars().all(|ch| ch.is_ascii_digit())
}

fn default_matches_prompt_type(value: &Value, prompt_type: PromptType) -> bool {
    match prompt_type {
        PromptType::String | PromptType::Paragraph | PromptType::Select => {
            matches!(value, Value::String(_))
        }
        PromptType::Bool => matches!(value, Value::Bool(_)),
        PromptType::MultiSelect => matches!(value, Value::Array(_)),
    }
}

fn literal_matches_dependency_type(value: &Value, prompt_type: PromptType) -> bool {
    match prompt_type {
        PromptType::String | PromptType::Paragraph | PromptType::Select => {
            matches!(value, Value::String(_))
        }
        PromptType::Bool => matches!(value, Value::Bool(_)),
        PromptType::MultiSelect => matches!(value, Value::String(_)),
    }
}

fn string_choices(prompt: &Prompt) -> HashSet<String> {
    prompt
        .choices
        .iter()
        .filter_map(|value| match value {
            Value::String(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}

fn dependency_graph(file: &AchitekFile) -> Graph<String> {
    let prompt_names = file
        .prompts()
        .iter()
        .map(|prompt| prompt.value.name.clone())
        .collect::<Vec<_>>();
    let prompt_name_set = prompt_names
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let edges = file
        .prompts()
        .iter()
        .flat_map(|prompt| {
            prompt
                .value
                .depends_on
                .as_ref()
                .into_iter()
                .flat_map(dependency_references)
                .filter(|reference| prompt_name_set.contains(*reference))
                .map(|reference| (reference.to_owned(), prompt.value.name.clone()))
        })
        .collect::<Vec<_>>();

    Graph {
        nodes: prompt_names,
        edges,
    }
}

fn dependency_references(dependency: &Dependency) -> Vec<&str> {
    let mut references = Vec::new();
    collect_dependency_references(dependency, &mut references);
    references
}

fn collect_dependency_references<'a>(dependency: &'a Dependency, references: &mut Vec<&'a str>) {
    match dependency {
        Dependency::Reference(reference) => references.push(reference),
        Dependency::Comparison { left, .. } => references.push(left),
        Dependency::Contains { receiver, .. } => references.push(receiver),
        Dependency::All(dependencies) | Dependency::Any(dependencies) => {
            for dependency in dependencies {
                collect_dependency_references(dependency, references);
            }
        }
    }
}

fn missing_node_code(node: Node<'_>) -> DiagnosticCode {
    match node.parent().map(|parent| parent.kind()) {
        Some("array" | "value_list") => return DiagnosticCode::MalformedArray,
        Some("depends_on_attribute" | "dependency_expr") => {
            return DiagnosticCode::InvalidDependencyExpression;
        }
        Some(
            "blueprint_attribute"
            | "type_attribute"
            | "help_attribute"
            | "choices_attribute"
            | "default_attribute"
            | "required_attribute"
            | "regex_attribute"
            | "min_length_attribute"
            | "max_length_attribute"
            | "min_selections_attribute"
            | "max_selections_attribute",
        ) => return DiagnosticCode::MissingAttributeValue,
        Some("prompt_block") if node.kind() == "string_literal" => {
            return DiagnosticCode::MissingPromptName;
        }
        Some("string_literal") => return DiagnosticCode::UnterminatedString,
        _ => {}
    }

    match node.kind() {
        "blueprint_block" => DiagnosticCode::MissingBlueprintBlock,
        "array" | "value_list" => DiagnosticCode::MalformedArray,
        "dependency_expr" => DiagnosticCode::InvalidDependencyExpression,
        "string_literal" => DiagnosticCode::UnterminatedString,
        "identifier" => DiagnosticCode::InvalidIdentifier,
        "integer" => DiagnosticCode::InvalidInteger,
        _ => DiagnosticCode::UnknownTopLevelItem,
    }
}

fn error_node_code(node: Node<'_>, source: &str) -> DiagnosticCode {
    let node_text = text(node, source);

    match node.parent().map(|parent| parent.kind()) {
        Some("array" | "value_list") => DiagnosticCode::MalformedArray,
        Some("string_literal") => DiagnosticCode::InvalidEscapeSequence,
        Some("required_attribute" | "boolean") => DiagnosticCode::InvalidBooleanLiteral,
        Some("type_attribute" | "question_type") => DiagnosticCode::UnknownPromptType,
        Some("method_call_dependency" | "method_name") => DiagnosticCode::UnknownDependencyMethod,
        Some("depends_on_attribute" | "dependency_expr") => {
            DiagnosticCode::InvalidDependencyExpression
        }
        Some("blueprint_block" | "blueprint_attribute") => {
            DiagnosticCode::UnknownBlueprintAttribute
        }
        Some("prompt_block" | "question_attribute") if node_text.trim_start().starts_with('.') => {
            DiagnosticCode::UnknownDependencyMethod
        }
        Some("prompt_block" | "question_attribute") => DiagnosticCode::UnknownPromptAttribute,
        Some("validate_block" | "validate_attribute") => DiagnosticCode::UnknownValidateAttribute,
        Some("file") if node_text.trim_start().starts_with("prompt {") => {
            DiagnosticCode::MissingPromptName
        }
        _ => DiagnosticCode::UnknownTopLevelItem,
    }
}

fn starts_with_keyword(text: &str, keyword: &str) -> bool {
    let text = text.trim_start();
    let Some(rest) = text.strip_prefix(keyword) else {
        return false;
    };

    rest.chars()
        .next()
        .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
}
