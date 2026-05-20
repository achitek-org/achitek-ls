use crate::utils;
use achitek_ls::{
    handlers::notification::handle_did_open,
    server::{ServerState, utils as server_utils},
};
use indoc::indoc;
use lsp_server::{Connection, Notification};
use lsp_types::{
    DiagnosticSeverity, DidOpenTextDocumentParams, NumberOrString, TextDocumentItem, Uri,
    notification::{DidOpenTextDocument, Notification as LspNotification},
};
use std::fs;

#[test]
fn did_open_stores_document_and_publishes_diagnostics() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri: Uri = "file:///workspace/Achitekfile".parse()?;
    let notification = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "achitekfile".to_owned(),
                version: 7,
                text: source(),
            },
        },
    );
    let mut state = ServerState::default();
    let params = serde_json::from_value(notification.params)?;

    handle_did_open(&server_connection, &mut state, params)?;

    let document = state
        .documents
        .get(uri.as_str())
        .expect("document should be stored");
    assert_eq!(document.version, 7);
    assert_eq!(document.text, source());

    let diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(diagnostics.uri, uri);
    assert_eq!(diagnostics.version, Some(7));
    assert!(diagnostics.diagnostics.is_empty());

    Ok(())
}

#[test]
fn opening_achitekfile_publishes_template_diagnostics() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-open-template-diagnostics")?;
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
    let notification = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "achitekfile".to_owned(),
                version: 1,
                text: source(),
            },
        },
    );
    let mut state = ServerState::default();
    let params = serde_json::from_value(notification.params)?;

    handle_did_open(&server_connection, &mut state, params)?;

    let achitek_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(achitek_diagnostics.uri, uri);
    assert!(achitek_diagnostics.diagnostics.is_empty());
    let template_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(template_diagnostics.uri, template_uri);
    assert_eq!(template_diagnostics.diagnostics.len(), 1);
    assert_eq!(
        template_diagnostics.diagnostics[0].message,
        "unknown prompt reference `missing_prompt`"
    );

    fs::remove_dir_all(&temp_root)?;
    Ok(())
}

#[test]
fn opening_template_publishes_achitekfile_diagnostics() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-open-achitekfile-diagnostics")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, source_with_prompt())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    fs::write(&template_path, "{{ project_name }}")?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let notification = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: template_uri.clone(),
                language_id: "tera".to_owned(),
                version: 1,
                text: String::new(),
            },
        },
    );
    let mut state = ServerState::default();
    let params = serde_json::from_value(notification.params)?;

    handle_did_open(&server_connection, &mut state, params)?;

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
fn opening_template_publishes_unknown_prompt_diagnostics() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-open-template-unknown-prompt")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, source())?;
    let template_path = temp_root.join("Cargo.toml.tera");
    let template_source = r#"name = "{{ missing_prompt }}""#;
    fs::write(&template_path, template_source)?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let notification = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: template_uri.clone(),
                language_id: "tera".to_owned(),
                version: 1,
                text: template_source.to_owned(),
            },
        },
    );
    let mut state = ServerState::default();
    let params = serde_json::from_value(notification.params)?;

    handle_did_open(&server_connection, &mut state, params)?;

    let template_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(template_diagnostics.uri, template_uri);
    assert_eq!(template_diagnostics.version, Some(1));
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

#[test]
fn opening_template_publishes_unknown_select_choice_warning() -> anyhow::Result<()> {
    let temp_root = utils::temp_dir("achitek-did-open-template-unknown-select-choice")?;
    fs::create_dir_all(&temp_root)?;
    let achitek_path = temp_root.join("Achitekfile");
    fs::write(&achitek_path, source_with_license())?;
    let template_path = temp_root.join("LICENSE.tera");
    let template_source = "{% if license == 'recommended' -%}Recommended{% endif %}";
    fs::write(&template_path, template_source)?;
    let achitek_uri = server_utils::path_to_uri(&achitek_path)?;
    let template_uri = server_utils::path_to_uri(&template_path)?;
    let (server_connection, client_connection) = Connection::memory();
    let notification = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: template_uri.clone(),
                language_id: "tera".to_owned(),
                version: 1,
                text: template_source.to_owned(),
            },
        },
    );
    let mut state = ServerState::default();
    let params = serde_json::from_value(notification.params)?;

    handle_did_open(&server_connection, &mut state, params)?;

    let template_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(template_diagnostics.uri, template_uri);
    assert_eq!(template_diagnostics.version, Some(1));
    assert_eq!(template_diagnostics.diagnostics.len(), 1);
    assert_eq!(
        template_diagnostics.diagnostics[0].code,
        Some(NumberOrString::String("ACHLS0003".to_owned()))
    );
    assert_eq!(
        template_diagnostics.diagnostics[0].severity,
        Some(DiagnosticSeverity::WARNING)
    );
    assert_eq!(
        template_diagnostics.diagnostics[0].message,
        "prompt `license` has no choice `recommended`"
    );
    let achitek_diagnostics = utils::published_diagnostics_sink_timeout(&client_connection)?;
    assert_eq!(achitek_diagnostics.uri, achitek_uri);

    fs::remove_dir_all(&temp_root)?;
    Ok(())
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

fn source_with_license() -> String {
    indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "license" {
          type = select
          help = "License"
          choices = ["MIT", "Apache-2.0", "GPL-2.0-only"]
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
