use achitek_ls::{
    handlers::notification::handle_did_close,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Notification};
use lsp_types::{
    DidCloseTextDocumentParams, TextDocumentIdentifier, Uri,
    notification::{DidCloseTextDocument, Notification as LspNotification},
};

use crate::utils;

#[test]
fn did_close_removes_document_and_clears_diagnostics() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = "file:///workspace/Achitekfile".parse()?;
    let notification = Notification::new(
        DidCloseTextDocument::METHOD.to_owned(),
        DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        },
    );
    let mut state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(notification.params)?;

    handle_did_close(&server_connection, &mut state, params)?;

    assert!(!state.documents.contains_key(uri.as_str()));
    let diagnostics = utils::published_diagnostics_sink(&client_connection)?;
    assert_eq!(diagnostics.uri, uri);
    assert_eq!(diagnostics.version, None);
    assert!(diagnostics.diagnostics.is_empty());

    Ok(())
}
