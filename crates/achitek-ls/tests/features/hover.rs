use crate::utils;
use achitek_ls::{
    handlers::request::handle_hover,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    Hover, HoverContents, HoverParams, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri,
    request::{HoverRequest, Request as LspRequest},
};

#[test]
fn hover_shows_prompt_type_details() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = Request::new(
        request_id.clone(),
        HoverRequest::METHOD.to_owned(),
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 6,
                    character: 9,
                },
            },
            work_done_progress_params: Default::default(),
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
    let result = handle_hover(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let hover: Option<Hover> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let hover = hover.expect("hover should be available");
    let HoverContents::Markup(contents) = hover.contents else {
        panic!("expected markup hover contents");
    };
    assert!(contents.value.contains("string"));

    Ok(())
}

#[test]
fn hover_returns_empty_for_unknown_document() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let request_id = RequestId::from(1_i32);
    let request = Request::new(
        request_id.clone(),
        HoverRequest::METHOD.to_owned(),
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: utils::TEST_URI.parse()?,
                },
                position: Position {
                    line: 6,
                    character: 9,
                },
            },
            work_done_progress_params: Default::default(),
        },
    );
    let state = ServerState::default();
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_hover(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let hover: Option<Hover> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    assert!(hover.is_none());

    Ok(())
}
