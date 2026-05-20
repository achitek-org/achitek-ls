//! Project-scoped diagnostics for blueprint relationships.
//!
//! Language crates own single-file diagnostics. This module owns LSP
//! diagnostics that require looking across a blueprint project, such as prompt
//! declarations in `achitekfile` versus references in `.tera` templates.

use crate::server::{
    ServerState,
    project::ProjectContext,
    utils::{self, TemplateReference},
};
use anyhow::Context;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::Node;

pub(crate) const UNKNOWN_PROMPT_CODE: &str = "ACHLS0001";
const UNUSED_PROMPT_CODE: &str = "ACHLS0002";
const UNKNOWN_PROMPT_CHOICE_CODE: &str = "ACHLS0003";

pub(crate) fn achitekfile_diagnostics(
    uri: &Uri,
    state: &ServerState,
) -> anyhow::Result<Vec<Diagnostic>> {
    let Some(project) = ProjectContext::for_uri(state, uri) else {
        return Ok(Vec::new());
    };

    let prompt_ranges = prompt_ranges(&project.achitekfile_source()?).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    if prompt_ranges.is_empty() {
        return Ok(Vec::new());
    }

    let used_prompts = template_references(&project)?
        .into_iter()
        .map(|reference| reference.name)
        .collect::<HashSet<_>>();

    Ok(prompt_ranges
        .into_iter()
        .filter(|(name, _range)| !used_prompts.contains(name))
        .map(|(name, range)| Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(UNUSED_PROMPT_CODE.to_owned())),
            message: format!("prompt `{name}` is not used by any template"),
            ..Diagnostic::default()
        })
        .collect())
}

pub(crate) fn template_diagnostics(
    uri: &Uri,
    state: &ServerState,
) -> anyhow::Result<Vec<(Uri, Vec<Diagnostic>)>> {
    let Some(project) = ProjectContext::for_uri(state, uri) else {
        return Ok(Vec::new());
    };

    let prompt_catalog = prompt_catalog(&project.achitekfile_source()?).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let prompt_names = prompt_catalog.keys().cloned().collect::<HashSet<_>>();

    let mut diagnostics = Vec::new();
    for template_path in template_paths(project.root())? {
        let template_uri = utils::path_to_uri(&template_path).with_context(|| {
            format!(
                "failed to convert `{}` to a file URI",
                template_path.display()
            )
        })?;
        let mut template_diagnostics = template_path_diagnostics(&template_path)?;
        template_diagnostics.extend(template_path_project_diagnostics(
            &template_path,
            &template_uri,
            &prompt_names,
            &prompt_catalog,
        )?);
        let source = project.template_source(&template_uri, &template_path)?;
        template_diagnostics.extend(
            tera_diagnostics(&source)
                .with_context(|| format!("failed to analyze `{}`", template_path.display()))?,
        );
        template_diagnostics.extend(unknown_prompt_diagnostics(
            &source,
            &template_uri,
            &prompt_names,
        ));
        template_diagnostics.extend(unknown_prompt_choice_diagnostics(
            &source,
            &prompt_catalog,
        )?);
        diagnostics.push((template_uri, template_diagnostics));
    }

    Ok(diagnostics)
}

pub(crate) fn template_project_diagnostics(
    uri: &Uri,
    state: &ServerState,
) -> anyhow::Result<Vec<Diagnostic>> {
    let Some(project) = ProjectContext::for_uri(state, uri) else {
        return Ok(Vec::new());
    };
    let Some(template_path) = utils::file_path_from_uri(uri) else {
        return Ok(Vec::new());
    };

    let prompt_catalog = prompt_catalog(&project.achitekfile_source()?).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let prompt_names = prompt_catalog.keys().cloned().collect::<HashSet<_>>();
    let source = project.template_source(uri, &template_path)?;

    let mut diagnostics = template_path_diagnostics(&template_path)?;
    diagnostics.extend(template_path_project_diagnostics(
        &template_path,
        uri,
        &prompt_names,
        &prompt_catalog,
    )?);
    diagnostics.extend(unknown_prompt_diagnostics(&source, uri, &prompt_names));
    diagnostics.extend(unknown_prompt_choice_diagnostics(
        &source,
        &prompt_catalog,
    )?);

    Ok(diagnostics)
}

fn prompt_ranges(source: &str) -> anyhow::Result<HashMap<String, Range>> {
    let analysis = achitekfile::analyze(source)?;
    Ok(analysis
        .file()
        .prompts()
        .iter()
        .map(|prompt| {
            (
                prompt.value.name.clone(),
                achitek_range_to_lsp(prompt.range),
            )
        })
        .collect())
}

fn prompt_catalog(source: &str) -> anyhow::Result<HashMap<String, PromptInfo>> {
    let analysis = achitekfile::analyze(source)?;
    Ok(analysis
        .file()
        .prompts()
        .iter()
        .map(|prompt| {
            (
                prompt.value.name.clone(),
                PromptInfo {
                    choices: select_choices(&prompt.value),
                },
            )
        })
        .collect())
}

fn template_references(project: &ProjectContext<'_>) -> anyhow::Result<Vec<TemplateReference>> {
    let mut references = Vec::new();
    for template_path in template_paths(project.root())? {
        let template_uri = utils::path_to_uri(&template_path).with_context(|| {
            format!(
                "failed to convert `{}` to a file URI",
                template_path.display()
            )
        })?;
        references.extend(template_path_references(&template_path, &template_uri));
        let source = project.template_source(&template_uri, &template_path)?;
        references.extend(utils::template_references_in_source(&source, &template_uri));
    }
    Ok(references)
}

fn template_path_references(path: &Path, uri: &Uri) -> Vec<TemplateReference> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };

    utils::template_references_in_path(file_name, uri)
}

fn template_paths(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_template_paths(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_template_paths(root: &Path, paths: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_template_paths(&path, paths)?;
        } else if utils::is_tera_path(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn tera_diagnostics(source: &str) -> anyhow::Result<Vec<Diagnostic>> {
    let analysis = terafile::analyze(source)?;

    Ok(analysis
        .diagnostics()
        .iter()
        .map(|diagnostic| Diagnostic {
            range: tera_range_to_lsp(diagnostic.range()),
            severity: Some(to_tera_lsp_severity(diagnostic.severity())),
            code: Some(NumberOrString::String(
                diagnostic.code().as_str().to_owned(),
            )),
            message: diagnostic.message().to_owned(),
            ..Diagnostic::default()
        })
        .collect())
}

fn template_path_diagnostics(path: &Path) -> anyhow::Result<Vec<Diagnostic>> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(Vec::new());
    };

    if !is_templated_path_text(file_name) {
        return Ok(Vec::new());
    }

    tera_diagnostics(file_name)
}

fn template_path_project_diagnostics(
    path: &Path,
    uri: &Uri,
    prompt_names: &HashSet<String>,
    prompt_catalog: &HashMap<String, PromptInfo>,
) -> anyhow::Result<Vec<Diagnostic>> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(Vec::new());
    };

    if !is_templated_path_text(file_name) {
        return Ok(Vec::new());
    }

    let mut diagnostics = unknown_prompt_diagnostics(file_name, uri, prompt_names);
    diagnostics.extend(unknown_prompt_choice_diagnostics(
        file_name,
        prompt_catalog,
    )?);

    Ok(diagnostics)
}

fn is_templated_path_text(value: &str) -> bool {
    value.contains("{{") || value.contains("{%") || value.contains("{#")
}

fn unknown_prompt_diagnostics(
    source: &str,
    uri: &Uri,
    prompt_names: &HashSet<String>,
) -> Vec<Diagnostic> {
    utils::template_references_in_source(source, uri)
        .into_iter()
        .filter(|reference| !prompt_names.contains(&reference.name))
        .map(|reference| Diagnostic {
            range: reference.location.range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(UNKNOWN_PROMPT_CODE.to_owned())),
            message: format!("unknown prompt reference `{}`", reference.name),
            ..Diagnostic::default()
        })
        .collect()
}

fn unknown_prompt_choice_diagnostics(
    source: &str,
    prompts: &HashMap<String, PromptInfo>,
) -> anyhow::Result<Vec<Diagnostic>> {
    Ok(template_choice_comparisons(source)?
        .into_iter()
        .filter(|comparison| {
            prompts
                .get(&comparison.prompt_name)
                .and_then(|prompt| prompt.choices.as_ref())
                .is_some_and(|choices| !choices.contains(&comparison.choice))
        })
        .map(|comparison| Diagnostic {
            range: comparison.choice_range,
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(
                UNKNOWN_PROMPT_CHOICE_CODE.to_owned(),
            )),
            message: format!(
                "prompt `{}` has no choice `{}`",
                comparison.prompt_name, comparison.choice
            ),
            ..Diagnostic::default()
        })
        .collect())
}

fn template_choice_comparisons(source: &str) -> anyhow::Result<Vec<ChoiceComparison>> {
    let tree = terafile::parse(source)?;
    let mut comparisons = Vec::new();
    collect_choice_comparisons(tree.root_node(), source, &mut comparisons);
    Ok(comparisons)
}

fn collect_choice_comparisons(
    node: Node<'_>,
    source: &str,
    comparisons: &mut Vec<ChoiceComparison>,
) {
    if node.kind() == "binary_expression" {
        if let Some(comparison) = choice_comparison(node, source) {
            comparisons.push(comparison);
        }
    }

    for child in children(node) {
        collect_choice_comparisons(child, source, comparisons);
    }
}

fn choice_comparison(node: Node<'_>, source: &str) -> Option<ChoiceComparison> {
    let operator = node.child_by_field_name("operator")?;
    if node_text(operator, source) != "==" {
        return None;
    }

    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name("right")?;

    prompt_choice_comparison(left, right, source)
        .or_else(|| prompt_choice_comparison(right, left, source))
}

fn prompt_choice_comparison(
    prompt_node: Node<'_>,
    choice_node: Node<'_>,
    source: &str,
) -> Option<ChoiceComparison> {
    if prompt_node.kind() != "identifier" || choice_node.kind() != "string" {
        return None;
    }

    Some(ChoiceComparison {
        prompt_name: node_text(prompt_node, source).to_owned(),
        choice: parse_template_string(choice_node, source)?,
        choice_range: tera_range_to_lsp(terafile::TextRange::from(choice_node.range())),
    })
}

fn children(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect::<Vec<_>>().into_iter()
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes())
        .expect("tree-sitter node byte ranges should be valid utf-8 slices")
}

fn parse_template_string(node: Node<'_>, source: &str) -> Option<String> {
    let raw = node_text(node, source);
    let quote = raw.chars().next()?;
    if !matches!(quote, '"' | '\'' | '`') || !raw.ends_with(quote) {
        return None;
    }

    Some(raw[quote.len_utf8()..raw.len() - quote.len_utf8()].to_owned())
}

fn select_choices(prompt: &achitekfile::model::Prompt) -> Option<HashSet<String>> {
    if prompt.prompt_type != Some(achitekfile::model::PromptType::Select) {
        return None;
    }

    Some(
        prompt
            .choices
            .iter()
            .filter_map(|choice| match choice {
                achitekfile::model::Value::String(value) => Some(value.clone()),
                _ => None,
            })
            .collect(),
    )
}

fn to_tera_lsp_severity(severity: terafile::Severity) -> DiagnosticSeverity {
    match severity {
        terafile::Severity::Error => DiagnosticSeverity::ERROR,
        terafile::Severity::Warning => DiagnosticSeverity::WARNING,
        terafile::Severity::Hint => DiagnosticSeverity::HINT,
    }
}

fn achitek_range_to_lsp(range: achitekfile::TextRange) -> Range {
    Range {
        start: achitek_position_to_lsp(range.start),
        end: achitek_position_to_lsp(range.end),
    }
}

fn achitek_position_to_lsp(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}

fn tera_range_to_lsp(range: terafile::TextRange) -> Range {
    Range {
        start: tera_position_to_lsp(range.start),
        end: tera_position_to_lsp(range.end),
    }
}

fn tera_position_to_lsp(position: terafile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}

#[derive(Debug)]
struct PromptInfo {
    choices: Option<HashSet<String>>,
}

#[derive(Debug)]
struct ChoiceComparison {
    prompt_name: String,
    choice: String,
    choice_range: Range,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Document, Documents};
    use indoc::indoc;

    #[test]
    fn achitekfile_diagnostics_reports_unused_prompts() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-unused-prompt")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, r#"name = "{{project_name}}""#)?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let state = ServerState {
            documents: Documents::from([(
                achitek_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: source(),
                },
            )]),
            ..Default::default()
        };

        let diagnostics = achitekfile_diagnostics(&achitek_uri, &state)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(UNUSED_PROMPT_CODE.to_owned()))
        );
        assert_eq!(
            diagnostics[0].message,
            "prompt `repository_name` is not used by any template"
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn template_diagnostics_reports_unknown_prompt_references() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-unknown-prompt")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, r#"name = "{{missing_prompt}}""#)?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState {
            documents: Documents::from([(
                achitek_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: source(),
                },
            )]),
            ..Default::default()
        };

        let diagnostics = template_diagnostics(&achitek_uri, &state)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].0, template_uri);
        assert_eq!(diagnostics[0].1.len(), 1);
        assert_eq!(
            diagnostics[0].1[0].code,
            Some(NumberOrString::String(UNKNOWN_PROMPT_CODE.to_owned()))
        );
        assert_eq!(
            diagnostics[0].1[0].message,
            "unknown prompt reference `missing_prompt`"
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn template_project_diagnostics_reports_unknown_prompt_references_for_one_template()
    -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-single-template-unknown-prompt")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, r#"name = "{{missing_prompt}}""#)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState::default();

        let diagnostics = template_project_diagnostics(&template_uri, &state)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(UNKNOWN_PROMPT_CODE.to_owned()))
        );
        assert_eq!(
            diagnostics[0].message,
            "unknown prompt reference `missing_prompt`"
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn template_project_diagnostics_ignore_tera_builtins_and_filter_arguments() -> anyhow::Result<()>
    {
        let temp_root = utils::temp_dir("achitek-project-template-builtins")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("README.md.tera");
        fs::write(
            &template_path,
            r#"Copyright {{now() | date(format="%Y")}} {{author}}"#,
        )?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState::default();

        let diagnostics = template_project_diagnostics(&template_uri, &state)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "unknown prompt reference `author`");

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn template_project_diagnostics_warns_for_unknown_select_choice() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-template-unknown-select-choice")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source_with_license())?;
        let template_path = temp_root.join("LICENSE.tera");
        fs::write(&template_path, "{% if license == 'recommended' -%}")?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState::default();

        let diagnostics = template_project_diagnostics(&template_uri, &state)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                UNKNOWN_PROMPT_CHOICE_CODE.to_owned()
            ))
        );
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            diagnostics[0].message,
            "prompt `license` has no choice `recommended`"
        );
        assert_eq!(diagnostics[0].range.start.character, 17);
        assert_eq!(diagnostics[0].range.end.character, 30);

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn template_project_diagnostics_allows_known_select_choice() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-template-known-select-choice")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source_with_license())?;
        let template_path = temp_root.join("LICENSE.tera");
        fs::write(&template_path, "{% if license == 'MIT' -%}")?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState::default();

        let diagnostics = template_project_diagnostics(&template_uri, &state)?;

        assert!(diagnostics.is_empty());

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn template_diagnostics_report_syntax_errors_in_templated_file_names() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-templated-file-name-syntax")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source_with_license())?;
        let template_path = temp_root.join("{% if license == 'recommended' %}LICENSE-MIT");
        fs::write(&template_path, "")?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState::default();

        let diagnostics = template_diagnostics(&achitek_uri, &state)?;
        let (_, template_diagnostics) = diagnostics
            .iter()
            .find(|(uri, _diagnostics)| *uri == template_uri)
            .expect("templated file name should receive diagnostics");

        assert!(template_diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("TERA0000".to_owned()))
                || diagnostic.code == Some(NumberOrString::String("TERA0001".to_owned()))
        }));

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn achitekfile_diagnostics_counts_prompt_references_in_templated_file_names()
    -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-project-templated-file-name-reference")?;
        fs::create_dir_all(temp_root.join("src"))?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source_with_kind())?;
        let template_path = temp_root
            .join("src/{% if kind == 'bin' %}main.rs.tera{% else %}lib.rs.tera{% endif %}");
        fs::write(&template_path, "")?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let state = ServerState::default();

        let diagnostics = achitekfile_diagnostics(&achitek_uri, &state)?;

        assert!(diagnostics.is_empty());

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    fn source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }

            prompt "repository_name" {
              type = string
            }
        "#}
        .to_owned()
    }

    fn source_with_kind() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = select
              help = "--bin or --lib"
              choices = ["bin", "lib"]
            }
        "#}
        .to_owned()
    }

    fn source_with_license() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "license" {
              type = select
              help = "License"
              choices = ["MIT", "Apache-2.0", "GPL-2.0-only"]
            }
        "#}
        .to_owned()
    }
}
