//! Handler for the LSP `textDocument/prepareRename` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_prepareRename>
//!
//! Clients send this request before showing rename UI. The response tells the
//! client whether the cursor is on a renameable symbol and which range should
//! be edited.

use crate::{
    editor,
    server::{ServerState, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{Position, PrepareRenameResponse, Range, TextDocumentPositionParams};
use std::fs;

/// Handles a `textDocument/prepareRename` request.
pub fn handle(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    match state.document_kind(&params.text_document.uri) {
        DocumentKind::Achitekfile => achitekfile_prepare_rename(state, params),
        DocumentKind::TeraTemplate => tera_prepare_rename(state, params),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_prepare_rename(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    let Some(document) = state.documents.get(params.text_document.uri.as_str()) else {
        return Ok(None);
    };

    let editor_buffer = editor::from_source(&document.text).with_context(|| {
        format!(
            "failed to analyze document `{:?}`",
            params.text_document.uri
        )
    })?;
    Ok(editor_buffer
        .prepare_rename(to_text_position(params.position))
        .map(|target| PrepareRenameResponse::RangeWithPlaceholder {
            range: to_lsp_range(target.range()),
            placeholder: target.placeholder().to_owned(),
        }))
}

fn tera_prepare_rename(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    let uri = params.text_document.uri;
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template prepare rename skipped for non-file URI");
        return Ok(None);
    };
    let source = state
        .documents
        .get(uri.as_str())
        .map(|document| Ok(document.text.clone()))
        .unwrap_or_else(|| {
            fs::read_to_string(&template_path)
                .with_context(|| format!("failed to read template `{}`", template_path.display()))
        })?;

    Ok(
        utils::reference_target_at_position(&source, params.position).map(
            |(placeholder, range)| PrepareRenameResponse::RangeWithPlaceholder {
                range,
                placeholder,
            },
        ),
    )
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
