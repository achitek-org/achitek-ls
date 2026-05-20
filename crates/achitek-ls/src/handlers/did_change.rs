//! Handler for the LSP `textDocument/didChange` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didChange>
//!
//! Clients send this notification after an open document changes. This server
//! uses full-document sync, so the latest change replaces the stored document
//! text before diagnostics are republished for both the document and nearby
//! `.tera` templates that reference its prompts.

use crate::lsp::publish;
use crate::server::ServerState;
use lsp_server::Connection;
use lsp_types::{DidChangeTextDocumentParams, TextDocumentContentChangeEvent};

/// Handles a `textDocument/didChange` notification.
pub fn handle(
    connection: &Connection,
    state: &mut ServerState,
    params: DidChangeTextDocumentParams,
) -> anyhow::Result<()> {
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    let change_count = params.content_changes.len();

    if let Some(document) = state.documents.get_mut(uri.as_str()) {
        document.version = version;
        document.text = apply_content_changes(&document.text, &params.content_changes);
        tracing::debug!(?uri, version, change_count, "changed document");
        publish::publish_after_document_update(connection, &uri, state)?;
    } else {
        tracing::warn!(
            ?uri,
            version,
            change_count,
            "received change for unknown document"
        );
    }

    Ok(())
}

/// Applies full-document content changes.
fn apply_content_changes(
    current_text: &str,
    content_changes: &[TextDocumentContentChangeEvent],
) -> String {
    content_changes
        .last()
        .map(|change| change.text.clone())
        .unwrap_or_else(|| current_text.to_owned())
}
