use achitek_ls::{
    handlers::notification::handle_did_change,
    server::{Document, Documents, ServerState, utils as server_utils},
};
use indoc::indoc;
use lsp_server::{Connection, Notification};
use lsp_types::{
    DidChangeTextDocumentParams, NumberOrString, TextDocumentContentChangeEvent, Uri,
    VersionedTextDocumentIdentifier,
    notification::{DidChangeTextDocument, Notification as LspNotification},
};
use std::fs;

use crate::utils;

#[test]
fn did_change_updates_document_and_publishes_diagnostics() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = "file:///workspace/Achitekfile".parse()?;
    let mut state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]),
        ..Default::default()
    };
    let notification = change_notification(uri.clone(), 2, source());
    let params = serde_json::from_value(notification.params)?;

    handle_did_change(&server_connection, &mut state, params)?;

    let document = state
        .documents
        .get(uri.as_str())
        .expect("document should remain stored");
    assert_eq!(document.version, 2);
    assert_eq!(document.text, source());

    let diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(diagnostics.uri, uri);
    assert_eq!(diagnostics.version, Some(2));
    assert!(diagnostics.diagnostics.is_empty());

    Ok(())
}

#[test]
fn changing_achitekfile_publishes_template_diagnostics() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-change-template-diagnostics")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, source())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    fs::write(
        &template_path,
        indoc! {r#"
            [package]
            name = "{{missing_prompt}}"
        "#},
    )?;
    let uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let mut state = ServerState {
        documents: Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]),
        ..Default::default()
    };
    let notification = change_notification(uri.clone(), 2, source());
    let params = serde_json::from_value(notification.params)?;

    handle_did_change(&server_connection, &mut state, params)?;

    let achitek_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(achitek_diagnostics.uri, uri);
    assert!(achitek_diagnostics.diagnostics.is_empty());
    let template_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(template_diagnostics.uri, template_uri);
    assert_eq!(template_diagnostics.diagnostics.len(), 1);

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

#[test]
fn changing_template_publishes_achitekfile_diagnostics() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-change-achitekfile-diagnostics")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, source_with_prompt())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    fs::write(&template_path, "{{ project_name }}")?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let mut state = ServerState {
        documents: Documents::from([(
            template_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: "{{ project_name }}".to_owned(),
            },
        )]),
        ..Default::default()
    };
    let notification = change_notification(template_uri.clone(), 2, String::new());
    let params = serde_json::from_value(notification.params)?;

    handle_did_change(&server_connection, &mut state, params)?;

    let template_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(template_diagnostics.uri, template_uri);
    let achitek_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(achitek_diagnostics.uri, achitek_uri);
    assert_eq!(achitek_diagnostics.diagnostics.len(), 1);
    assert_eq!(
        achitek_diagnostics.diagnostics[0].message,
        "prompt `project_name` is not used by any template"
    );

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

#[test]
fn changing_template_publishes_unknown_prompt_diagnostics() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-change-template-unknown-prompt")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, source())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    fs::write(&template_path, "")?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let template_source = r#"name = "{{ missing_prompt }}""#;
    let (server_connection, client_connection) = Connection::memory();
    let mut state = ServerState {
        documents: Documents::from([(
            template_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: String::new(),
            },
        )]),
        ..Default::default()
    };
    let notification = change_notification(template_uri.clone(), 2, template_source.to_owned());
    let params = serde_json::from_value(notification.params)?;

    handle_did_change(&server_connection, &mut state, params)?;

    let template_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(template_diagnostics.uri, template_uri);
    assert_eq!(template_diagnostics.version, Some(2));
    assert_eq!(template_diagnostics.diagnostics.len(), 1);
    assert_eq!(
        template_diagnostics.diagnostics[0].code,
        Some(NumberOrString::String("ACHLS0001".to_owned()))
    );
    assert_eq!(
        template_diagnostics.diagnostics[0].message,
        "unknown prompt reference `missing_prompt`"
    );
    let achitek_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(achitek_diagnostics.uri, achitek_uri);

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

fn change_notification(uri: Uri, version: i32, text: String) -> Notification {
    Notification::new(
        DidChangeTextDocument::METHOD.to_owned(),
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text,
            }],
        },
    )
}

fn source() -> String {
    indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }
    "#}
    .to_owned()
}

fn source_with_prompt() -> String {
    indoc! {r#"
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
