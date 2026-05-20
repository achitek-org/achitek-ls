//! Handler for the LSP `textDocument/completion` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_completion>
//!
//! Clients send this request when they need completion items at a cursor
//! position. For Achitekfiles, completions include DSL keywords, attributes,
//! prompt types, references, and dependency-expression helpers.

use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Position, Uri,
};

/// Handles a `textDocument/completion` request.
pub fn handle(
    state: &ServerState,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => achitekfile_completions(state, uri, position),
        DocumentKind::TeraTemplate => tera_completions(state, uri, position),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_completions(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<CompletionResponse>> {
    if let Some(document) = state.documents.get(uri.as_str()) {
        let editor_buffer = editor::from_source(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let items = editor_buffer
            .completions(to_text_position(position))
            .into_iter()
            .map(to_lsp_completion_item)
            .collect::<Vec<_>>();

        Ok(Some(CompletionResponse::Array(items)))
    } else {
        Ok(None)
    }
}

fn tera_completions(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<CompletionResponse>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template completion skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template completion skipped because no project was found"
        );
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;
    if !utils::is_template_expression_position(&source, position) {
        return Ok(None);
    }

    let achitek_source = project.achitekfile_source()?;
    let editor_buffer = editor::from_source(&achitek_source).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let items = editor_buffer
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == editor::SymbolKind::Prompt)
        .map(|symbol| CompletionItem {
            label: symbol.name().to_owned(),
            detail: Some("Prompt reference".to_owned()),
            kind: Some(CompletionItemKind::REFERENCE),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    Ok(Some(CompletionResponse::Array(items)))
}

/// Converts an editor completion into an LSP completion item.
fn to_lsp_completion_item(item: editor::Completion) -> CompletionItem {
    CompletionItem {
        label: item.label().to_owned(),
        detail: item.detail().map(str::to_owned),
        kind: Some(match item.kind() {
            editor::CompletionKind::Keyword => CompletionItemKind::KEYWORD,
            editor::CompletionKind::Property => CompletionItemKind::PROPERTY,
            editor::CompletionKind::Value => CompletionItemKind::VALUE,
            editor::CompletionKind::Reference => CompletionItemKind::REFERENCE,
            editor::CompletionKind::Function => CompletionItemKind::FUNCTION,
        }),
        ..CompletionItem::default()
    }
}

/// Converts an LSP position into an editor position.
fn to_text_position(position: Position) -> achitekfile::TextPosition {
    achitekfile::TextPosition {
        line: usize::try_from(position.line).expect("line should fit into usize"),
        byte: usize::try_from(position.character).expect("character should fit into usize"),
    }
}
