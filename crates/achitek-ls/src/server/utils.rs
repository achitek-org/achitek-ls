//! Shared utilities for the server.
//!
//! Achitek templates are ordinary `.tera` files that can reference prompt names
//! declared in a nearby `Achitekfile`. These helpers preserve the cross-file
//! behavior used by diagnostics, go-to-definition, references, and rename.

use anyhow::Context;
use lsp_types::{Location, Position, Range, Uri};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    template_references_in_source(source, uri)
        .into_iter()
        .filter(|reference| reference.name == prompt_name)
        .map(|reference| reference.location)
        .collect()
}

pub(crate) fn reference_at_position(source: &str, position: Position) -> Option<String> {
    reference_target_at_position(source, position).map(|reference| reference.0)
}

pub(crate) fn reference_target_at_position(
    source: &str,
    position: Position,
) -> Option<(String, Range)> {
    let column = usize::try_from(position.character).ok()?;

    template_references_in_source(source, &"file:///template.tera".parse().ok()?)
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
        .map(|reference| (reference.name, reference.location.range))
}

pub(crate) fn is_template_expression_position(source: &str, position: Position) -> bool {
    let row = usize::try_from(position.line).ok();
    let column = usize::try_from(position.character).ok();
    let Some((line, column)) = row.and_then(|row| source.lines().nth(row)).zip(column) else {
        return false;
    };
    if column > line.len() {
        return false;
    }

    let before = &line[..column];
    let in_output = before
        .rfind("{{")
        .is_some_and(|open| before[open..].find("}}").is_none());
    let in_tag = before
        .rfind("{%")
        .is_some_and(|open| before[open..].find("%}").is_none())
        && before.chars().filter(|ch| *ch == '"').count() % 2 == 0;

    in_output || in_tag
}

pub(crate) fn template_references_in_source(source: &str, uri: &Uri) -> Vec<TemplateReference> {
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

pub(crate) fn find_achitekfile_for_template(template_path: &Path) -> Option<PathBuf> {
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

pub(crate) fn is_tera_path(path: &Path) -> bool {
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

#[derive(Debug, Clone)]
pub(crate) struct TemplateReference {
    pub(crate) name: String,
    pub(crate) location: Location,
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
}
