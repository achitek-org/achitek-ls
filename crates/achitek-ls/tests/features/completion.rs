use achitek_ls::{
    handlers::request::handle_completion,
    server::{Document, Documents, ServerState},
};
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    CompletionParams, CompletionResponse, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri,
    request::{Completion, Request as LspRequest},
};
use std::fs;

use crate::utils;

#[test]
fn completion_returns_achitekfile_prompt_attributes() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = utils::TEST_URI.parse()?;
    let request_id = RequestId::from(1_i32);
    let request = Request::new(
        request_id.clone(),
        Completion::METHOD.to_owned(),
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 6,
                    character: 2,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
    );
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: valid_completion_source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_completion(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    assert_eq!(response.id, request_id);
    assert!(response.error.is_none());

    let result: Option<CompletionResponse> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let Some(CompletionResponse::Array(items)) = result else {
        panic!("expected completion item array");
    };
    assert!(items.iter().any(|item| item.label == "type"));

    Ok(())
}

#[test]
fn completion_returns_prompt_names_in_tera_expressions() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-template-completion")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, valid_completion_source())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    let template_source = "{{ pro".to_owned();
    fs::write(&template_path, &template_source)?;
    let uri = achitek_ls::server::utils::path_to_uri(&template_path)?;
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 0,
                character: 6,
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: template_source,
            },
        )]),
        ..Default::default()
    };

    let Some(CompletionResponse::Array(items)) = handle_completion(&state, params)? else {
        panic!("expected completion item array");
    };

    assert!(items.iter().any(|item| item.label == "project_name"));

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

fn valid_completion_source() -> String {
    indoc::indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "project_name" {
          
        }
    "#}
    .to_owned()
}
