//! Handler for the LSP `textDocument/rename` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_rename>
//!
//! Clients send this request after the user confirms a new symbol name. For
//! Achitekfiles, this returns a workspace edit that renames a prompt declaration,
//! its document-local references, and matching prompt references in nearby
//! `.tera` templates.
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{Position, Range, RenameParams, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;

/// Handles a `textDocument/rename` request.
#[allow(clippy::mutable_key_type)]
pub fn handle(state: &ServerState, params: RenameParams) -> anyhow::Result<Option<WorkspaceEdit>> {
    let text_document_position = params.text_document_position;
    let uri = text_document_position.text_document.uri;
    let position = text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => achitekfile_rename(state, uri, position, &params.new_name),
        DocumentKind::TeraTemplate => tera_rename(state, uri, position, &params.new_name),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

#[allow(clippy::mutable_key_type)]
fn achitekfile_rename(
    state: &ServerState,
    uri: Uri,
    position: Position,
    new_name: &str,
) -> anyhow::Result<Option<WorkspaceEdit>> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(None);
    };

    let editor_buffer = editor::from_source(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let cursor_position = to_text_position(position);
    let Some(prompt_name) = editor_buffer.prompt_name(cursor_position) else {
        return Ok(None);
    };
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    add_achitekfile_edits(
        &mut changes,
        uri.clone(),
        &document.text,
        &editor_buffer,
        cursor_position,
        new_name,
    );

    if let Some(project) = ProjectContext::for_uri(state, &uri) {
        add_template_edits(&mut changes, &project, &prompt_name, new_name)?;
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

#[allow(clippy::mutable_key_type)]
fn tera_rename(
    state: &ServerState,
    uri: Uri,
    position: Position,
    new_name: &str,
) -> anyhow::Result<Option<WorkspaceEdit>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template rename skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(?uri, "template rename skipped because no project was found");
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;
    let Some(prompt_name) = utils::reference_at_position(&source, position) else {
        tracing::debug!(?uri, ?position, "no template reference under cursor");
        return Ok(None);
    };

    let achitek_uri = project.achitekfile_uri()?;
    let achitek_source = project.achitekfile_source()?;
    let editor_buffer = editor::from_source(&achitek_source).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let Some(symbol) = editor_buffer
        .symbols()
        .iter()
        .find(|symbol| symbol.kind() == editor::SymbolKind::Prompt && symbol.name() == prompt_name)
    else {
        tracing::debug!(
            ?uri,
            reference = prompt_name,
            target = ?achitek_uri,
            "template rename skipped because prompt was not found"
        );
        return Ok(None);
    };

    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    add_achitekfile_edits(
        &mut changes,
        achitek_uri,
        &achitek_source,
        &editor_buffer,
        symbol.selection_range().start,
        new_name,
    );

    add_template_edits(&mut changes, &project, &prompt_name, new_name)?;

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

#[allow(clippy::mutable_key_type)]
fn add_achitekfile_edits(
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    uri: Uri,
    source: &str,
    analysis: &editor::EditorBuffer,
    position: achitekfile::TextPosition,
    new_name: &str,
) {
    for target in analysis.references(position, true) {
        let range = to_lsp_range(target.range());
        let replacement = replacement_text_for_range(source, &range, new_name);
        changes.entry(uri.clone()).or_default().push(TextEdit {
            range,
            new_text: replacement,
        });
    }
}

#[allow(clippy::mutable_key_type)]
fn add_template_edits(
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    project: &ProjectContext<'_>,
    prompt_name: &str,
    new_name: &str,
) -> anyhow::Result<()> {
    for location in project.scan_template_references(prompt_name)? {
        let Some(path) = utils::file_path_from_uri(&location.uri) else {
            continue;
        };
        let source = project.template_source(&location.uri, &path)?;
        let replacement = replacement_text_for_range(&source, &location.range, new_name);
        changes.entry(location.uri).or_default().push(TextEdit {
            range: location.range,
            new_text: replacement,
        });
    }

    Ok(())
}

fn replacement_text_for_range(source: &str, range: &Range, new_name: &str) -> String {
    if selected_text(source, range).is_some_and(|text| text.starts_with('"') && text.ends_with('"'))
    {
        format!("\"{new_name}\"")
    } else {
        new_name.to_owned()
    }
}

fn selected_text<'a>(source: &'a str, range: &Range) -> Option<&'a str> {
    if range.start.line != range.end.line {
        return None;
    }

    let line = source
        .lines()
        .nth(usize::try_from(range.start.line).ok()?)?;
    let start = usize::try_from(range.start.character).ok()?;
    let end = usize::try_from(range.end.character).ok()?;
    line.get(start..end)
}

fn to_text_position(position: Position) -> achitekfile::TextPosition {
    achitekfile::TextPosition {
        line: usize::try_from(position.line).expect("line should fit into usize"),
        byte: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

fn to_lsp_range(range: achitekfile::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start),
        end: to_lsp_position(range.end),
    }
}

fn to_lsp_position(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}
