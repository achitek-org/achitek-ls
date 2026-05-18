//! Handler for the LSP `textDocument/didChange` notification.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didChange>
//!
//! Clients send this notification after an open document changes. This server
//! uses full-document sync, so the latest change replaces the stored document
//! text before diagnostics are republished for both the document and nearby
//! `.tera` templates that reference its prompts.

use crate::lsp::publish;
#[cfg(test)]
use crate::server::Document;
#[cfg(test)]
use crate::server::Documents;
use crate::server::ServerState;
use lsp_server::Connection;
#[cfg(test)]
use lsp_types::Uri;
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
        publish::publish(connection, &uri, state)?;
        publish::publish_templates(connection, &uri, state)?;
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::utils;
    use indoc::indoc;
    use lsp_server::{Connection, Message, Notification};
    use lsp_types::{
        PublishDiagnosticsParams, VersionedTextDocumentIdentifier,
        notification::{DidChangeTextDocument, Notification as LspNotification},
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
    fn handle_did_change_updates_document_and_publishes_diagnostics() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let mut documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]);
        let notification = Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: source(),
                }],
            },
        );

        handle(&server_connection, &notification, &mut documents)?;

        let document = documents
            .get(uri.as_str())
            .expect("document should remain stored");
        assert_eq!(document.version, 2);
        assert_eq!(document.text, source());

        let diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(diagnostics.uri, uri);
        assert_eq!(diagnostics.version, Some(2));
        assert!(diagnostics.diagnostics.is_empty());

        Ok(())
    }

    #[test]
    fn handle_did_change_publishes_template_diagnostics() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-did-change-template-diagnostics")?;
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
        let mut documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]);
        let notification = Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: source(),
                }],
            },
        );

        handle(&server_connection, &notification, &mut documents)?;

        let achitek_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(achitek_diagnostics.uri, uri);
        assert!(achitek_diagnostics.diagnostics.is_empty());
        let template_diagnostics = recv_publish_diagnostics(&client_connection)?;
        assert_eq!(template_diagnostics.uri, template_uri);
        assert_eq!(template_diagnostics.diagnostics.len(), 1);

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
