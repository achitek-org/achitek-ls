use achitek_ls::{
    handlers::request::handle_definition,
    server::{Document, Documents, ServerState, utils as server_utils},
    workspace::Workspace,
};
use indoc::indoc;
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri,
    request::{GotoDefinition, Request as LspRequest},
};
use std::fs;

use crate::utils;

#[test]
fn definition_resolves_achitekfile_prompt_reference() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = "file:///workspace/Achitekfile".parse()?;
    let request = Request::new(
        RequestId::from(1_i32),
        GotoDefinition::METHOD.to_owned(),
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 13,
                    character: 16,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let documents = Documents::from([(
        uri.as_str().to_owned(),
        Document {
            version: 1,
            text: reference_source(),
        },
    )]);
    let state = ServerState {
        documents,
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_definition(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let location = scalar_definition(response.result.expect("response should contain a result"))?;
    assert_eq!(location.range.start.line, 5);

    Ok(())
}

#[test]
fn definition_resolves_tera_prompt_reference() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-template-definition")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, reference_source())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    fs::write(
        &template_path,
        indoc! {r#"[package]
            name = "{{project_name}}"
        "#},
    )?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let request = Request::new(
        RequestId::from(1_i32),
        GotoDefinition::METHOD.to_owned(),
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: template_uri },
                position: Position {
                    line: 1,
                    character: 13,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let documents = Documents::from([(
        achitek_uri.as_str().to_owned(),
        Document {
            version: 1,
            text: reference_source(),
        },
    )]);
    let state = ServerState {
        documents,
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_definition(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let location = scalar_definition(response.result.expect("response should contain a result"))?;
    assert_eq!(location.uri, achitek_uri);
    assert_eq!(location.range.start.line, 5);

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

#[test]
fn definition_uses_manifest_discovered_project_for_open_template() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-workspace-template-definition")?;
    let project_root = temp_root.join("rust");
    fs::create_dir_all(&project_root)?;
    fs::write(
        temp_root.join("blueprints.toml"),
        indoc! {r#"
            [rust]
            path = "./rust"
        "#},
    )?;
    let achitek_path = project_root.join("achitekfile");
    fs::write(&achitek_path, reference_source())?;
    let template_path = project_root.join("Cargo.toml.tera");
    fs::write(&template_path, "")?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let state = ServerState {
        documents: Documents::from([
            (
                achitek_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: reference_source(),
                },
            ),
            (
                template_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: indoc! {r#"[package]
                        name = "{{project_name}}"
                    "#}
                    .to_owned(),
                },
            ),
        ]),
        workspace: Workspace::discover(&temp_root)?,
        ..Default::default()
    };

    let result = handle_definition(
        &state,
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: template_uri },
                position: Position {
                    line: 1,
                    character: 13,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    )?;

    let Some(GotoDefinitionResponse::Scalar(location)) = result else {
        panic!("expected scalar definition response");
    };
    assert_eq!(location.uri, achitek_uri);
    assert_eq!(location.range.start.line, 5);

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

fn scalar_definition(result: serde_json::Value) -> anyhow::Result<lsp_types::Location> {
    let result: Option<GotoDefinitionResponse> = serde_json::from_value(result)?;
    let Some(GotoDefinitionResponse::Scalar(location)) = result else {
        anyhow::bail!("expected scalar definition response");
    };
    Ok(location)
}

fn reference_source() -> String {
    indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "project_name" {
          type = string
          help = "Project name"
        }

        prompt "kind" {
          type = string
          help = "Kind"
          depends_on = project_name
        }
    "#}
    .to_owned()
}
