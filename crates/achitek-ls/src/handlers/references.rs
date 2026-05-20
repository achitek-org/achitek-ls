//! Handler for the LSP `textDocument/references` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_references>
//!
//! Clients send this request when the user asks to find all uses of the symbol
//! at a cursor position. For Achitekfiles, this returns prompt declaration and
//! dependency-expression reference locations, plus prompt references in nearby
//! `.tera` templates.
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{Location, Position, Range, ReferenceParams, Uri};

/// Handles a `textDocument/references` request.
pub fn handle(
    state: &ServerState,
    params: ReferenceParams,
) -> anyhow::Result<Option<Vec<Location>>> {
    let text_document_position = params.text_document_position;
    let uri = text_document_position.text_document.uri;
    let position = text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => {
            achitekfile_references(state, uri, position, params.context.include_declaration)
        }
        DocumentKind::TeraTemplate => {
            tera_references(state, uri, position, params.context.include_declaration)
        }
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_references(
    state: &ServerState,
    uri: Uri,
    position: Position,
    include_declaration: bool,
) -> anyhow::Result<Option<Vec<Location>>> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(None);
    };

    let editor_buffer = editor::from_source(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let cursor_position = to_text_position(position);
    let prompt_name = editor_buffer.prompt_name(cursor_position);
    let mut locations = editor_buffer
        .references(cursor_position, include_declaration)
        .into_iter()
        .map(|target| Location::new(uri.clone(), to_lsp_range(target.range())))
        .collect::<Vec<_>>();

    if let (Some(prompt_name), Some(project)) = (prompt_name, ProjectContext::for_uri(state, &uri))
    {
        locations.extend(project.scan_template_references(&prompt_name)?);
    }

    Ok(Some(locations))
}

fn tera_references(
    state: &ServerState,
    uri: Uri,
    position: Position,
    include_declaration: bool,
) -> anyhow::Result<Option<Vec<Location>>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template references skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template references skipped because no project was found"
        );
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
            "template references skipped because prompt was not found"
        );
        return Ok(None);
    };

    let cursor_position = symbol.selection_range().start;
    let mut locations = editor_buffer
        .references(cursor_position, include_declaration)
        .into_iter()
        .map(|target| Location::new(achitek_uri.clone(), to_lsp_range(target.range())))
        .collect::<Vec<_>>();

    locations.extend(project.scan_template_references(&prompt_name)?);

    Ok(Some(locations))
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
