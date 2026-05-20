use crate::utils;
use achitek_ls::{
    handlers::request::handle_document_symbol,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    DocumentSymbolParams, DocumentSymbolResponse, TextDocumentIdentifier, Uri,
    request::{DocumentSymbolRequest, Request as LspRequest},
};

#[test]
fn document_symbol_returns_achitekfile_outline() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = Request::new(
        request_id.clone(),
        DocumentSymbolRequest::METHOD.to_owned(),
        DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: utils::achitekfile_with_prompt(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_document_symbol(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let symbols: Option<DocumentSymbolResponse> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let DocumentSymbolResponse::Nested(symbols) =
        symbols.expect("document symbols should be available")
    else {
        panic!("expected nested document symbols");
    };

    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].name, "blueprint");
    assert_eq!(symbols[1].name, "project_name");
    assert_eq!(symbols[1].children.as_ref().map(Vec::len), Some(0));

    Ok(())
}

#[test]
fn document_symbol_returns_empty_for_unknown_document() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let request_id = RequestId::from(1_i32);
    let request = Request::new(
        request_id.clone(),
        DocumentSymbolRequest::METHOD.to_owned(),
        DocumentSymbolParams {
            text_document: TextDocumentIdentifier {
                uri: utils::TEST_URI.parse()?,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_document_symbol(&ServerState::default(), params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let symbols: Option<DocumentSymbolResponse> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    assert!(symbols.is_none());

    Ok(())
}
