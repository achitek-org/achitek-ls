use crate::protocol::{self, Document, Documents};
use anyhow::Context;
use lsp_server::{Connection, Notification};
use lsp_types::DidOpenTextDocumentParams;

use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location,
    Position, PublishDiagnosticsParams, Range, Uri,
};

/// Handler for the LSP `textDocument/didOpen` notification.
///
/// [Spec]
///
/// Clients send this notification after a document is opened. The server stores
/// the in-memory text so later requests operate on the editor buffer rather
/// than stale file contents, then publishes diagnostics for that document and
/// for nearby `.tera` templates that reference its prompts.
/// particular textDocument is one
///
/// [Spec]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didOpen
pub fn handle(
    connection: &Connection,
    notification: &Notification,
    documents: &mut Documents,
) -> anyhow::Result<()> {
    let params: DidOpenTextDocumentParams = serde_json::from_value(notification.params.clone())
        .context("failed to parse didOpen params")?;
    let file = params.text_document;
    let uri = file.uri;

    // Open and close notification must be balanced and the max open count
    // for a particular textDocument is one. Warn in the case of this violaiton.
    if documents.contains_key(uri.as_str()) {
        tracing::warn!("received duplicate didOpen; replacing stored document")
    }
    documents.insert(
        uri.as_str().to_owned(),
        Document {
            version: file.version,
            text: file.text,
        },
    );
    tracing::debug!(?uri, file.version, "opened document");

    protocol::diagnostics::publish(connection, &uri, documents)?;

    Ok(())
}
