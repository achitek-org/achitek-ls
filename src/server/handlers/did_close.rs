//! Handler for the LSP `textDocument/didClose` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didClose>
//!
//! Clients send this notification after a document is closed. The server drops
//! the in-memory buffer and clears diagnostics for that URI.

use super::diagnostics;
use crate::server::Document;
use anyhow::Context;
use lsp_server::{Connection, Notification};
use lsp_types::{DidCloseTextDocumentParams, Uri};
use std::collections::HashMap;

/// Handles a `textDocument/didClose` notification.
pub fn handle(
    connection: &Connection,
    notification: &Notification,
    documents: &mut HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: DidCloseTextDocumentParams = serde_json::from_value(notification.params.clone())
        .context("failed to parse didClose params")?;
    let uri = params.text_document.uri;

    if documents.remove(&uri).is_some() {
        tracing::debug!(?uri, "closed document");
    } else {
        tracing::warn!(?uri, "received close for unknown document");
    }
    diagnostics::clear(connection, &uri)
}

#[cfg(test)]
mod test {
    use super::*;
    use lsp_server::Message;
    use lsp_types::{
        PublishDiagnosticsParams, TextDocumentIdentifier,
        notification::{DidCloseTextDocument, Notification as LspNotification},
    };

    #[test]
    fn handle_did_close_removes_document_and_clears_diagnostics() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let notification = Notification::new(
            DidCloseTextDocument::METHOD.to_owned(),
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            },
        );
        let mut documents = HashMap::from([(
            uri.clone(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]);

        handle(&server_connection, &notification, &mut documents)?;

        assert!(!documents.contains_key(&uri));
        let diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(diagnostics.uri, uri);
        assert_eq!(diagnostics.version, None);
        assert!(diagnostics.diagnostics.is_empty());

        Ok(())
    }

    fn recv_publish_diagnostics(
        connection: &Connection,
    ) -> anyhow::Result<PublishDiagnosticsParams> {
        match connection.receiver.recv()? {
            Message::Notification(notification)
                if notification.method == "textDocument/publishDiagnostics" =>
            {
                Ok(serde_json::from_value(notification.params)?)
            }
            message => anyhow::bail!("expected publishDiagnostics, got {message:?}"),
        }
    }

    fn test_uri() -> anyhow::Result<Uri> {
        Ok("file:///workspace/Achitekfile".parse()?)
    }
}
