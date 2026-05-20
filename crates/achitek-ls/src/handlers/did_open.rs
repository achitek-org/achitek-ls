//! Handler for the LSP `textDocument/didOpen` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didOpen>
//!
//! Clients send this notification after a document is opened. The server stores
//! the in-memory text so later requests operate on the editor buffer rather
//! than stale file contents, then publishes diagnostics for that document and
//! for nearby `.tera` templates that reference its prompts.

use crate::lsp::publish;
use crate::server::{Document, ServerState};
use lsp_server::Connection;
use lsp_types::DidOpenTextDocumentParams;

/// Handles a `textDocument/didOpen` notification.
pub fn handle(
    connection: &Connection,
    state: &mut ServerState,
    params: DidOpenTextDocumentParams,
) -> anyhow::Result<()> {
    let text_document = params.text_document;
    let uri = text_document.uri;
    let version = text_document.version;
    let language_id = text_document.language_id;

    state.documents.insert(
        uri.as_str().to_owned(),
        Document {
            version,
            text: text_document.text,
        },
    );
    state.set_document_kind(&uri, Some(&language_id), None);
    tracing::debug!(?uri, version, "opened document");
    publish::publish_after_document_update(connection, &uri, state)
}
