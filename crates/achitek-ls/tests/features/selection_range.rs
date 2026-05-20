use crate::utils;
use achitek_ls::{
    handlers::request::handle_selection_range,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    Position, SelectionRange, SelectionRangeParams, TextDocumentIdentifier, Uri,
    request::{Request as LspRequest, SelectionRangeRequest},
};

#[test]
fn selection_range_expands_from_prompt_name_to_prompt_block() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = selection_range_request(
        request_id.clone(),
        uri.clone(),
        vec![Position {
            line: 5,
            character: 10,
        }],
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
    let result = handle_selection_range(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let ranges: Option<Vec<SelectionRange>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let ranges = ranges.expect("selection ranges should be available");

    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].range.start.line, 5);
    assert_eq!(ranges[0].range.start.character, 7);
    assert_eq!(ranges[0].range.end.line, 5);
    assert_eq!(ranges[0].range.end.character, 21);

    let parent = ranges[0]
        .parent
        .as_ref()
        .expect("selection range should have a parent");
    assert_eq!(parent.range.start.line, 5);
    assert_eq!(parent.range.start.character, 0);
    assert_eq!(parent.range.end.line, 8);

    Ok(())
}

#[test]
fn selection_range_returns_empty_for_unknown_document() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let request_id = RequestId::from(1_i32);
    let request = selection_range_request(
        request_id.clone(),
        utils::TEST_URI.parse()?,
        vec![Position {
            line: 5,
            character: 10,
        }],
    );
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_selection_range(&ServerState::default(), params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let ranges: Option<Vec<SelectionRange>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    assert!(ranges.is_none());

    Ok(())
}

fn selection_range_request(id: RequestId, uri: Uri, positions: Vec<Position>) -> Request {
    Request::new(
        id,
        SelectionRangeRequest::METHOD.to_owned(),
        SelectionRangeParams {
            text_document: TextDocumentIdentifier { uri },
            positions,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    )
}
