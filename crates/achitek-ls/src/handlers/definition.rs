//! Handler for the LSP `textDocument/definition` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition>
//!
//! Clients send this request when the user asks to jump from a reference to the
//! source location that defines it. For Achitekfiles, this currently resolves
//! prompt references back to their prompt declarations. For `.tera` templates,
//! this can jump from a prompt interpolation back to the matching Achitekfile
//! prompt declaration.
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, Uri};

/// Handles a `textDocument/definition` request.
pub fn handle(
    state: &ServerState,
    params: GotoDefinitionParams,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let text_document_position = params.text_document_position_params;
    let uri = text_document_position.text_document.uri;
    let position = text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => achitekfile_definition(state, uri, position),
        DocumentKind::TeraTemplate => tera_definition(state, uri, position),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_definition(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(None);
    };

    let editor_buffer = editor::from_source(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;

    Ok(editor_buffer
        .definition(to_text_position(position))
        .map(|target| {
            GotoDefinitionResponse::Scalar(Location::new(
                uri,
                to_lsp_range(target.selection_range()),
            ))
        }))
}

fn tera_definition(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template definition skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template definition skipped because no project was found"
        );
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;

    let Some(reference_name) = utils::reference_at_position(&source, position) else {
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
    let Some(symbol) = editor_buffer.symbols().iter().find(|symbol| {
        symbol.kind() == editor::SymbolKind::Prompt && symbol.name() == reference_name
    }) else {
        tracing::debug!(
            ?uri,
            reference = reference_name,
            target = ?achitek_uri,
            "template definition skipped because prompt was not found"
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
        to_lsp_range(symbol.selection_range()),
    ))))
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
