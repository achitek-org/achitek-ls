use achitek_ls::{
    handlers::request::handle_rename,
    server::{Document, Documents, ServerState, utils as server_utils},
    workspace::Workspace,
};
use indoc::indoc;
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    Position, RenameParams, TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkspaceEdit,
    request::{Rename, Request as LspRequest},
};
use std::fs;

use crate::utils;

#[test]
#[allow(clippy::mutable_key_type)]
fn rename_updates_achitekfile_prompt_references() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = "file:///workspace/Achitekfile".parse()?;
    let request = Request::new(
        RequestId::from(1_i32),
        Rename::METHOD.to_owned(),
        RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 13,
                    character: 16,
                },
            },
            new_name: "repository".to_owned(),
            work_done_progress_params: Default::default(),
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
    let result = handle_rename(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let edit = workspace_edit(response.result.expect("response should contain a result"))?;
    let changes = edit.changes.expect("workspace edit should contain changes");
    let edits = changes.get(&uri).expect("uri should have edits");
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().any(|edit| edit.new_text == "\"repository\""));
    assert!(edits.iter().any(|edit| edit.new_text == "repository"));

    Ok(())
}

#[test]
#[allow(clippy::mutable_key_type)]
fn rename_includes_tera_prompt_references() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-template-rename")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, reference_source())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    fs::write(
        &template_path,
        indoc! {r#"[package]
            name = "{{project_name}}"
            repository = "{{project_name}}"
        "#},
    )?;
    let uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let request = Request::new(
        RequestId::from(1_i32),
        Rename::METHOD.to_owned(),
        RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 5,
                    character: 10,
                },
            },
            new_name: "repository_name".to_owned(),
            work_done_progress_params: Default::default(),
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
    let result = handle_rename(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let edit = workspace_edit(response.result.expect("response should contain a result"))?;
    let changes = edit.changes.expect("workspace edit should contain changes");
    let achitek_edits = changes.get(&uri).expect("Achitekfile should have edits");
    assert_eq!(achitek_edits.len(), 2);
    assert!(
        achitek_edits
            .iter()
            .any(|edit| edit.new_text == "\"repository_name\"")
    );
    assert!(
        achitek_edits
            .iter()
            .any(|edit| edit.new_text == "repository_name")
    );
    let template_edits = changes
        .get(&template_uri)
        .expect("template should have edits");
    assert_eq!(template_edits.len(), 2);
    assert!(
        template_edits
            .iter()
            .all(|edit| edit.new_text == "repository_name")
    );

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

#[test]
#[allow(clippy::mutable_key_type)]
fn template_rename_uses_manifest_discovered_project() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-workspace-template-rename")?;
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
            repository = "{{project_name}}"
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
                        repository = "{{project_name}}"
                    "#}
                    .to_owned(),
                },
            ),
        ]),
        workspace: Workspace::discover(&temp_root)?,
        ..Default::default()
    };

    let edit = handle_rename(
        &state,
        RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: template_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: 13,
                },
            },
            new_name: "repository_name".to_owned(),
            work_done_progress_params: Default::default(),
        },
    )?
    .expect("workspace edit should be available");
    let changes = edit.changes.expect("workspace edit should contain changes");

    let achitek_edits = changes
        .get(&achitek_uri)
        .expect("achitekfile should have edits");
    assert_eq!(achitek_edits.len(), 2);
    assert!(
        achitek_edits
            .iter()
            .any(|edit| edit.new_text == "\"repository_name\"")
    );
    assert!(
        achitek_edits
            .iter()
            .any(|edit| edit.new_text == "repository_name")
    );
    let template_edits = changes
        .get(&template_uri)
        .expect("template should have edits");
    assert_eq!(template_edits.len(), 2);
    assert!(
        template_edits
            .iter()
            .all(|edit| edit.new_text == "repository_name")
    );

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

fn workspace_edit(result: serde_json::Value) -> anyhow::Result<WorkspaceEdit> {
    let edit: Option<WorkspaceEdit> = serde_json::from_value(result)?;
    Ok(edit.expect("workspace edit should be available"))
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
