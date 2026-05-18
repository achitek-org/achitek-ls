//! Shared utilities for the server.
//!
//! Achitek templates are ordinary `.tera` files that can reference prompt names
//! declared in a nearby `Achitekfile`. These helpers preserve the cross-file
//! behavior used by diagnostics, go-to-definition, references, and rename.

#[cfg(test)]
use crate::server::Document;
use crate::workspace::Workspace;
use crate::{editor, server::Documents};
use anyhow::Context;
use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, GotoDefinitionResponse, Location,
    NumberOrString, Position, Range, Uri,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub fn diagnostics(
    uri: &Uri,
    documents: &Documents,
    workspace: &Workspace,
) -> anyhow::Result<Vec<(Uri, Vec<LspDiagnostic>)>> {
    if is_tera_uri(uri) {
        return Ok(Vec::new());
    }

    let Some(document) = documents.get(uri.as_str()) else {
        return Ok(Vec::new());
    };
    let Some(blueprint_dir) = workspace
        .project_for_uri(uri)
        .map(|project| project.root().to_path_buf())
        .or_else(|| blueprint_dir_from_uri(uri))
    else {
        return Ok(Vec::new());
    };

    let analysis = achitekfile::analyze(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let prompt_names = prompt_name_set(&analysis);
    tracing::debug!(
        ?uri,
        prompt_count = prompt_names.len(),
        directory = %blueprint_dir.display(),
        "scanning template diagnostics"
    );

    scan_diagnostics(&blueprint_dir, &prompt_names)
}

pub fn definition(
    uri: &Uri,
    position: Position,
    documents: &Documents,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let Some(template_path) = file_path_from_uri(uri) else {
        tracing::debug!(?uri, "template definition skipped for non-file URI");
        return Ok(None);
    };
    if template_path.extension().and_then(|ext| ext.to_str()) != Some("tera") {
        tracing::debug!(?uri, path = %template_path.display(), "template definition skipped for non-template file");
        return Ok(None);
    }

    let source = fs::read_to_string(&template_path)
        .with_context(|| format!("failed to read template `{}`", template_path.display()))?;
    let Some(reference_name) = reference_at_position(&source, position) else {
        tracing::debug!(
            ?uri,
            line = position.line,
            character = position.character,
            "no template reference at position"
        );
        return Ok(None);
    };
    let Some(achitek_path) = find_achitekfile_for_template(&template_path) else {
        tracing::warn!(?uri, path = %template_path.display(), "could not find Achitekfile for template");
        return Ok(None);
    };

    let achitek_uri = path_to_uri(&achitek_path)?;
    let achitek_source = documents
        .get(achitek_uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&achitek_path).unwrap_or_default());
    let analysis = editor::build(&achitek_source)
        .with_context(|| format!("failed to analyze `{}`", achitek_path.display()))?;
    let Some(symbol) = analysis.symbols().iter().find(|symbol| {
        symbol.kind() == editor::SymbolKind::Prompt && symbol.name() == reference_name
    }) else {
        tracing::debug!(
            ?uri,
            reference = reference_name,
            "template reference has no matching prompt"
        );
        return Ok(None);
    };
    tracing::debug!(
        ?uri,
        reference = reference_name,
        target = ?achitek_uri,
        "resolved template definition"
    );

    Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
        achitek_uri,
        Range {
            start: to_lsp_position(symbol.selection_range().start_position),
            end: to_lsp_position(symbol.selection_range().end_position),
        },
    ))))
}

pub fn scan_references(root: &Path, prompt_name: &str) -> anyhow::Result<Vec<Location>> {
    if !root.exists() {
        tracing::debug!(directory = %root.display(), prompt = prompt_name, "template reference scan skipped for missing directory");
        return Ok(Vec::new());
    }

    let mut locations = Vec::new();
    collect_references(root, prompt_name, &mut locations)?;
    tracing::debug!(
        directory = %root.display(),
        prompt = prompt_name,
        count = locations.len(),
        "scanned template references"
    );
    Ok(locations)
}

fn scan_diagnostics(
    root: &Path,
    prompt_names: &HashSet<String>,
) -> anyhow::Result<Vec<(Uri, Vec<LspDiagnostic>)>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut diagnostics = Vec::new();
    collect_diagnostics(root, prompt_names, &mut diagnostics)?;
    Ok(diagnostics)
}

fn collect_diagnostics(
    root: &Path,
    prompt_names: &HashSet<String>,
    diagnostics: &mut Vec<(Uri, Vec<LspDiagnostic>)>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_diagnostics(&path, prompt_names, diagnostics)?;
            continue;
        }

        if !is_tera_path(&path) {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template `{}`", path.display()))?;
        let uri = path_to_uri(&path)
            .with_context(|| format!("failed to convert `{}` to a file URI", path.display()))?;
        let mut template_diagnostics = tera_diagnostics(&source)?;
        template_diagnostics.extend(unknown_references(&source, &uri, prompt_names));
        diagnostics.push((uri.clone(), template_diagnostics));
    }

    Ok(())
}

fn tera_diagnostics(source: &str) -> anyhow::Result<Vec<LspDiagnostic>> {
    let analysis = terafile::analyze(source).context("failed to analyze Tera template")?;

    Ok(analysis
        .diagnostics()
        .iter()
        .map(|diagnostic| LspDiagnostic {
            range: tera_range_to_lsp(diagnostic.range()),
            severity: Some(match diagnostic.severity() {
                terafile::Severity::Error => DiagnosticSeverity::ERROR,
                terafile::Severity::Warning => DiagnosticSeverity::WARNING,
                terafile::Severity::Hint => DiagnosticSeverity::HINT,
            }),
            code: Some(NumberOrString::String(
                diagnostic.code().as_str().to_owned(),
            )),
            message: diagnostic.message().to_owned(),
            ..LspDiagnostic::default()
        })
        .collect())
}

fn unknown_references(
    source: &str,
    uri: &Uri,
    prompt_names: &HashSet<String>,
) -> Vec<LspDiagnostic> {
    identifiers_in_source(source, uri)
        .into_iter()
        .filter(|reference| !prompt_names.contains(&reference.name))
        .map(|reference| LspDiagnostic {
            range: reference.location.range,
            severity: Some(DiagnosticSeverity::ERROR),
            message: format!("unknown prompt reference `{}`", reference.name),
            ..LspDiagnostic::default()
        })
        .collect()
}

fn collect_references(
    root: &Path,
    prompt_name: &str,
    locations: &mut Vec<Location>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_references(&path, prompt_name, locations)?;
            continue;
        }

        if !is_tera_path(&path) {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template `{}`", path.display()))?;
        let uri = path_to_uri(&path)
            .with_context(|| format!("failed to convert `{}` to a file URI", path.display()))?;
        locations.extend(references_in_source(&source, &uri, prompt_name));
    }

    Ok(())
}

fn references_in_source(source: &str, uri: &Uri, prompt_name: &str) -> Vec<Location> {
    identifiers_in_source(source, uri)
        .into_iter()
        .filter(|reference| reference.name == prompt_name)
        .map(|reference| reference.location)
        .collect()
}

fn reference_at_position(source: &str, position: Position) -> Option<String> {
    let column = usize::try_from(position.character).ok()?;

    identifiers_in_source(source, &"file:///template.tera".parse().ok()?)
        .into_iter()
        .find(|reference| {
            let range = reference.location.range;
            range.start.line == position.line
                && usize::try_from(range.start.character)
                    .ok()
                    .is_some_and(|start| start <= column)
                && usize::try_from(range.end.character)
                    .ok()
                    .is_some_and(|end| column <= end)
        })
        .map(|reference| reference.name)
}

fn identifiers_in_source(source: &str, uri: &Uri) -> Vec<TemplateReference> {
    let mut references = Vec::new();

    for (row, line) in source.lines().enumerate() {
        let mut index = 0;
        while index < line.len() {
            let Some((offset, ch)) = line[index..].char_indices().next() else {
                break;
            };
            let start = index + offset;

            if !is_identifier_start(ch) {
                index = start + ch.len_utf8();
                continue;
            }

            let mut end = start + ch.len_utf8();
            while end < line.len() {
                let Some(next) = line[end..].chars().next() else {
                    break;
                };
                if !is_identifier_continue(next) {
                    break;
                }
                end += next.len_utf8();
            }

            let name = &line[start..end];
            if tera_reference_context(line, start, end)
                && !is_tera_keyword(name)
                && !(tera_tag_context(line, start, end)
                    && is_inside_quoted_template_string(line, start, end))
            {
                references.push(TemplateReference {
                    name: name.to_owned(),
                    location: Location::new(
                        uri.clone(),
                        Range {
                            start: Position {
                                line: u32::try_from(row).expect("row should fit into u32"),
                                character: u32::try_from(start)
                                    .expect("column should fit into u32"),
                            },
                            end: Position {
                                line: u32::try_from(row).expect("row should fit into u32"),
                                character: u32::try_from(end).expect("column should fit into u32"),
                            },
                        },
                    ),
                });
            }

            index = end;
        }
    }

    references
}

fn tera_reference_context(line: &str, start: usize, end: usize) -> bool {
    let before = &line[..start];
    let after = &line[end..];

    let in_output = before
        .rfind("{{")
        .is_some_and(|open| before[open..].find("}}").is_none())
        && after.contains("}}");
    let in_tag = before
        .rfind("{%")
        .is_some_and(|open| before[open..].find("%}").is_none())
        && after.contains("%}");

    in_output || in_tag
}

fn tera_tag_context(line: &str, start: usize, end: usize) -> bool {
    let before = &line[..start];
    let after = &line[end..];
    before
        .rfind("{%")
        .is_some_and(|open| before[open..].find("%}").is_none())
        && after.contains("%}")
}

fn is_inside_quoted_template_string(line: &str, start: usize, end: usize) -> bool {
    line[..start].chars().filter(|ch| *ch == '"').count() % 2 == 1 && line[end..].contains('"')
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_tera_keyword(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "as"
            | "block"
            | "elif"
            | "else"
            | "endblock"
            | "endfor"
            | "endif"
            | "extends"
            | "false"
            | "filter"
            | "for"
            | "if"
            | "in"
            | "include"
            | "loop"
            | "not"
            | "or"
            | "set"
            | "true"
            | "with"
    )
}

fn prompt_name_set(analysis: &achitekfile::Analysis<'_>) -> HashSet<String> {
    analysis
        .file()
        .prompts()
        .iter()
        .map(|prompt| prompt.value.name.clone())
        .collect()
}

fn find_achitekfile_for_template(template_path: &Path) -> Option<PathBuf> {
    let mut dir = template_path.parent()?;
    loop {
        let candidate = dir.join("Achitekfile");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

pub fn blueprint_dir_from_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = if cfg!(windows) && path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    Path::new(path).parent().map(Path::to_path_buf)
}

pub fn file_path_from_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = if cfg!(windows) && path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    Some(PathBuf::from(path))
}

pub fn is_tera_uri(uri: &Uri) -> bool {
    file_path_from_uri(uri).is_some_and(|path| is_tera_path(&path))
}

fn is_tera_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("tera")
}

pub fn path_to_uri(path: &Path) -> anyhow::Result<Uri> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize `{}`", path.display()))?;
    let value = format!("file://{}", path.to_string_lossy());
    value
        .parse()
        .with_context(|| format!("failed to parse `{value}` as a URI"))
}

fn to_lsp_position(position: crate::syntax::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.row).expect("line should fit into u32"),
        character: u32::try_from(position.column).expect("column should fit into u32"),
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

#[derive(Debug, Clone)]
struct TemplateReference {
    name: String,
    location: Location,
}

/// Returns a unique temporary directory path for a server test.
///
/// This helper is meant only for tests. The directory is not created
/// automatically; callers should create it with `fs::create_dir_all` and remove
/// it when the test is done.
#[cfg(test)]
pub fn temp_dir(prefix: &str) -> anyhow::Result<PathBuf> {
    Ok(std::env::temp_dir().join(format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    )))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::utils;
    use indoc::indoc;

    #[test]
    fn scan_references_finds_prompt_uses_in_template_files() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-references")?;
        fs::create_dir_all(&temp_root)?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"
                [package]
                name = "{{project}}"
                repository = "{{repo}}"

                {% if dev_profile == "FastCompile" -%}
                [profile.dev]
                debug = 0
                {% endif %}
            "#},
        )?;

        let references = scan_references(&temp_root, "repo")?;

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].range.start.line, 2);
        assert_eq!(references[0].range.start.character, 16);

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn diagnostics_reports_unknown_template_references() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-diagnostics")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"
                [package]
                name = "{{project_name}}"
                description = "{{missing_prompt}}"
            "#},
        )?;
        let achitek_uri = path_to_uri(&achitek_path)?;
        let template_uri = path_to_uri(&template_path)?;
        let documents = Documents::from([(
            achitek_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source(),
            },
        )]);

        let diagnostics = diagnostics(&achitek_uri, &documents, &Workspace::default())?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].0, template_uri);
        assert_eq!(diagnostics[0].1.len(), 1);
        assert_eq!(
            diagnostics[0].1[0].message,
            "unknown prompt reference `missing_prompt`"
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn diagnostics_includes_tera_template_diagnostics() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-tera-diagnostics")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, "name = {{ project_name")?;
        let achitek_uri = path_to_uri(&achitek_path)?;
        let template_uri = path_to_uri(&template_path)?;
        let documents = Documents::from([(
            achitek_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source(),
            },
        )]);

        let diagnostics = diagnostics(&achitek_uri, &documents, &Workspace::default())?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].0, template_uri);
        assert_eq!(diagnostics[0].1.len(), 1);
        assert_eq!(
            diagnostics[0].1[0].code,
            Some(NumberOrString::String("TERA0001".to_owned()))
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn diagnostics_skips_template_uri_rescans() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-rescan-skip")?;
        fs::create_dir_all(&temp_root)?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, "{{ missing_prompt }}")?;
        let template_uri = path_to_uri(&template_path)?;
        let documents = Documents::from([(
            template_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: "{{ missing_prompt }}".to_owned(),
            },
        )]);

        let diagnostics = diagnostics(&template_uri, &documents, &Workspace::default())?;

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
        "#}
        .to_owned()
    }
}
