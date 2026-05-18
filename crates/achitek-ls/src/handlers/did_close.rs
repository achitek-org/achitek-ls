//! Handler for the LSP `textDocument/didClose` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didClose>
//!
//! Clients send this notification after a document is closed. The server drops
//! the in-memory buffer and clears diagnostics for that URI.

use crate::lsp::publish;
#[cfg(test)]
use crate::server::Document;
#[cfg(test)]
use crate::server::Documents;
use crate::server::ServerState;
use lsp_server::Connection;
use lsp_types::DidCloseTextDocumentParams;
#[cfg(test)]
use lsp_types::Uri;

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

#[cfg(test)]
mod test {
    use super::*;
    use lsp_server::{Connection, Message, Notification};
    use lsp_types::{
        PublishDiagnosticsParams, TextDocumentIdentifier,
        notification::{DidCloseTextDocument, Notification as LspNotification},
    };

    fn handle(
        connection: &Connection,
        notification: &Notification,
        documents: &mut Documents,
    ) -> anyhow::Result<()> {
        let params = serde_json::from_value(notification.params.clone())?;
        let mut state = ServerState {
            documents: std::mem::take(documents),
            document_kinds: Default::default(),
            workspace: Default::default(),
        };
        let result = super::handle(connection, &mut state, params);
        *documents = state.documents;
        result
    }

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
        let mut documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]);

        handle(&server_connection, &notification, &mut documents)?;

        assert!(!documents.contains_key(uri.as_str()));
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
