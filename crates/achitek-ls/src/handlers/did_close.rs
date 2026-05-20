//! Handler for the LSP `textDocument/didClose` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didClose>
//!
//! Clients send this notification after a document is closed. The server drops
//! the in-memory buffer and clears diagnostics for that URI.

use crate::lsp::publish;
use crate::server::ServerState;
use lsp_server::Connection;
use lsp_types::DidCloseTextDocumentParams;

/// Handles a `textDocument/didClose` notification.
pub fn handle(
    connection: &Connection,
    state: &mut ServerState,
    params: DidCloseTextDocumentParams,
) -> anyhow::Result<()> {
    let uri = params.text_document.uri;

    if state.documents.remove(uri.as_str()).is_some() {
        state.remove_document_kind(&uri);
        tracing::debug!(?uri, "closed document");
    } else {
        tracing::warn!(?uri, "received close for unknown document");
    }
    publish::clear(connection, &uri)
}
