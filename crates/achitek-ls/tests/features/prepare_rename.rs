use achitek_ls::{
    handlers::request::handle_prepare_rename,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    Position, PrepareRenameResponse, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    request::{PrepareRenameRequest, Request as LspRequest},
};

use crate::utils;

#[test]
fn prepare_rename_accepts_achitekfile_prompt_name() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = Request::new(
        request_id.clone(),
        PrepareRenameRequest::METHOD.to_owned(),
        TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 10,
            },
        },
    );
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_prepare_rename(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let result: Option<PrepareRenameResponse> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) = result else {
        panic!("expected range with placeholder");
    };
    assert_eq!(placeholder, "project_name");

    Ok(())
}

#[test]
fn prepare_rename_accepts_tera_prompt_reference() -> anyhow::Result<()> {
    let uri: Uri = "file:///workspace/rust/Cargo.toml.tera".parse()?;
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: indoc::indoc! {r#"[package]
                    name = "{{project_name}}"
                "#}
                .to_owned(),
            },
        )]),
        ..Default::default()
    };

    let result = handle_prepare_rename(
        &state,
        TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 13,
            },
        },
    )?;

    let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, range }) = result else {
        panic!("expected range with placeholder");
    };
    assert_eq!(placeholder, "project_name");
    assert_eq!(range.start.line, 1);
    assert_eq!(range.start.character, 10);

    Ok(())
}

fn source() -> String {
    indoc::indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "project_name" {
          type = string
        }
    "#}
    .to_owned()
}
