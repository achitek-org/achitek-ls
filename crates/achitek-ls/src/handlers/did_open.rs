//! Handler for the LSP `textDocument/didOpen` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didOpen>
//!
//! Clients send this notification after a document is opened. The server stores
//! the in-memory text so later requests operate on the editor buffer rather
//! than stale file contents, then publishes diagnostics for that document and
//! for nearby `.tera` templates that reference its prompts.

use crate::lsp::publish;
#[cfg(test)]
use crate::server::Documents;
use crate::server::{Document, ServerState};
use lsp_server::Connection;
use lsp_types::DidOpenTextDocumentParams;
#[cfg(test)]
use lsp_types::Uri;

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
    publish::publish(connection, &uri, state)?;
    publish::publish_templates(connection, &uri, state)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::utils;
    use indoc::indoc;
    use lsp_server::{Connection, Message, Notification};
    use lsp_types::{
        PublishDiagnosticsParams, TextDocumentItem,
        notification::{DidOpenTextDocument, Notification as LspNotification},
    };
    use std::fs;

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
    fn handle_did_open_stores_document_and_publishes_diagnostics() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let notification = Notification::new(
            DidOpenTextDocument::METHOD.to_owned(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "achitekfile".to_owned(),
                    version: 7,
                    text: source(),
                },
            },
        );
        let mut documents = Documents::new();

        handle(&server_connection, &notification, &mut documents)?;

        let document = documents
            .get(uri.as_str())
            .expect("document should be stored");
        assert_eq!(document.version, 7);
        assert_eq!(document.text, source());

        let diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(diagnostics.uri, uri);
        assert_eq!(diagnostics.version, Some(7));
        assert!(diagnostics.diagnostics.is_empty());

        Ok(())
    }

    #[test]
    fn handle_did_open_publishes_template_diagnostics() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-did-open-template-diagnostics")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"
                [package]
                name = "{{missing_prompt}}"
            "#},
        )?;
        let uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let notification = Notification::new(
            DidOpenTextDocument::METHOD.to_owned(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "achitekfile".to_owned(),
                    version: 1,
                    text: source(),
                },
            },
        );
        let mut documents = Documents::new();

        handle(&server_connection, &notification, &mut documents)?;

        let achitek_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(achitek_diagnostics.uri, uri);
        assert!(achitek_diagnostics.diagnostics.is_empty());
        let template_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(template_diagnostics.uri, template_uri);
        assert_eq!(template_diagnostics.diagnostics.len(), 1);
        assert_eq!(
            template_diagnostics.diagnostics[0].message,
            "unknown prompt reference `missing_prompt`"
        );

        fs::remove_dir_all(&temp_root)?;
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

    fn source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }
        "#}
        .to_owned()
    }
}
