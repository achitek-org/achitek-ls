use crate::protocol::{Documents, diagnostics};
use anyhow::Context;
use lsp_server::{Connection, Notification};
use lsp_types::DidCloseTextDocumentParams;

/// Handler for the LSP `textDocument/didClose` notification
///
/// [Spec]
///
/// Clients send this notification after the document is closed.
///
/// [Spec]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didClose
pub fn handle(
    connection: &Connection,
    notification: &Notification,
    documents: &mut Documents,
) -> anyhow::Result<()> {
    let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params.clone())
        .context("failed to parse didClose params")?;
    let uri = params.text_document.uri;
    if documents.remove(uri.as_str()).is_some() {
        tracing::debug!(?uri, "closed document");
    } else {
        tracing::warn!(?uri, "received close for unknown document");
    }

    diagnostics::clear(connection, &uri)?;

    Ok(())
}
