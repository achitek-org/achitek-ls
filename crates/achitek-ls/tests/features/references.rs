use achitek_ls::{
    handlers::request::handle_references,
    server::{Document, Documents, ServerState, utils as server_utils},
    workspace::Workspace,
};
use indoc::indoc;
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    Location, Position, ReferenceContext, ReferenceParams, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri,
    request::{References, Request as LspRequest},
};
use std::fs;

use crate::utils;

#[test]
fn references_resolve_achitekfile_prompt_references() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = "file:///workspace/Achitekfile".parse()?;
    let request = Request::new(
        RequestId::from(1_i32),
        References::METHOD.to_owned(),
        ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 13,
                    character: 16,
                },
            },
            context: ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: reference_source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_references(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let locations =
        reference_locations(response.result.expect("response should contain a result"))?;
    assert_eq!(locations.len(), 2);
    assert!(
        locations
            .iter()
            .any(|location| location.range.start.line == 5)
    );
    assert!(
        locations
            .iter()
            .any(|location| location.range.start.line == 13)
    );

    Ok(())
}

#[test]
fn references_include_tera_prompt_references() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-template-references")?;
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
    let uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let request = Request::new(
        RequestId::from(1_i32),
        References::METHOD.to_owned(),
        ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 5,
                    character: 10,
                },
            },
            context: ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: reference_source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_references(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let locations =
        reference_locations(response.result.expect("response should contain a result"))?;
    assert_eq!(locations.len(), 3);
    assert!(
        locations
            .iter()
            .any(|location| location.uri == template_uri && location.range.start.line == 1)
    );

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

#[test]
fn template_references_use_manifest_discovered_project() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-workspace-template-references")?;
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
    fs::write(
        &template_path,
        indoc! {r#"[package]
            name = "{{project_name}}"
        "#},
    )?;
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

    let locations = handle_references(
        &state,
        ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: template_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: 13,
                },
            },
            context: ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    )?
    .expect("references should be available");

    assert_eq!(locations.len(), 3);
    assert!(
        locations
            .iter()
            .any(|location| location.uri == achitek_uri && location.range.start.line == 5)
    );
    assert!(
        locations
            .iter()
            .any(|location| location.uri == template_uri && location.range.start.line == 1)
    );

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

fn reference_locations(result: serde_json::Value) -> anyhow::Result<Vec<Location>> {
    let locations: Option<Vec<Location>> = serde_json::from_value(result)?;
    Ok(locations.expect("references should be available"))
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
