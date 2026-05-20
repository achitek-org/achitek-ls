use crate::utils;
use achitek_ls::{
    handlers::request::handle_folding_range,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    FoldingRange, FoldingRangeParams, TextDocumentIdentifier, Uri,
    request::{FoldingRangeRequest, Request as LspRequest},
};

#[test]
fn folding_range_returns_foldable_achitekfile_blocks() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = folding_range_request(request_id.clone(), uri.clone());
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source_with_validate(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_folding_range(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let ranges: Option<Vec<FoldingRange>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let ranges = ranges.expect("folding ranges should be available");

    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 0
                && range.collapsed_text.as_deref() == Some("blueprint"))
    );
    assert!(
        ranges.iter().any(|range| range.start_line == 5
            && range.collapsed_text.as_deref() == Some("project_name"))
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 8
                && range.collapsed_text.as_deref() == Some("validate"))
    );

    Ok(())
}

#[test]
fn folding_range_returns_empty_for_unknown_document() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let request_id = RequestId::from(1_i32);
    let request = folding_range_request(request_id.clone(), utils::TEST_URI.parse()?);
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_folding_range(&ServerState::default(), params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let ranges: Option<Vec<FoldingRange>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    assert!(ranges.is_none());

    Ok(())
}

fn folding_range_request(id: RequestId, uri: Uri) -> Request {
    Request::new(
        id,
        FoldingRangeRequest::METHOD.to_owned(),
        FoldingRangeParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    )
}

fn source_with_validate() -> String {
    indoc::indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "project_name" {
          type = string
          help = "Project name"
          validate {
            min_length = 2
          }
        }
    "#}
    .to_owned()
}
