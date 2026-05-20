use crate::utils;
use achitek_ls::{
    handlers::request::handle_formatting,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    DocumentFormattingParams, FormattingOptions, TextDocumentIdentifier, TextEdit, Uri,
    request::{Formatting, Request as LspRequest},
};

#[test]
fn formatting_returns_full_document_edit() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = formatting_request(request_id.clone(), uri.clone());
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: unformatted_source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_formatting(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let edits: Option<Vec<TextEdit>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let edits = edits.expect("formatting edits should be available");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, formatted_source());

    Ok(())
}

#[test]
fn formatting_returns_empty_edits_when_document_is_formatted() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = formatting_request(request_id.clone(), uri.clone());
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: formatted_source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_formatting(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let edits: Option<Vec<TextEdit>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    assert_eq!(edits, Some(Vec::new()));

    Ok(())
}

#[test]
fn formatting_returns_empty_for_unknown_document() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let request_id = RequestId::from(1_i32);
    let request = formatting_request(request_id.clone(), utils::TEST_URI.parse()?);
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_formatting(&ServerState::default(), params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let edits: Option<Vec<TextEdit>> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    assert!(edits.is_none());

    Ok(())
}

fn formatting_request(id: RequestId, uri: Uri) -> Request {
    Request::new(
        id,
        Formatting::METHOD.to_owned(),
        DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options: FormattingOptions {
                tab_size: 2,
                insert_spaces: true,
                ..FormattingOptions::default()
            },
            work_done_progress_params: Default::default(),
        },
    )
}

fn unformatted_source() -> String {
    indoc::indoc! {r#"
        blueprint {
            version = "1.0.0"
            name = "minimal"
        }

        prompt "project_name" {
            type = string
            help = "Project name"
        }
    "#}
    .to_owned()
}

fn formatted_source() -> String {
    indoc::indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "project_name" {
          type = string
          help = "Project name"
        }
    "#}
    .to_owned()
}
