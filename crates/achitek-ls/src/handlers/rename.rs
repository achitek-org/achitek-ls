//! Handler for the LSP `textDocument/rename` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_rename>
//!
//! Clients send this request after the user confirms a new symbol name. For
//! Achitekfiles, this returns a workspace edit that renames a prompt declaration,
//! its document-local references, and matching prompt references in nearby
//! `.tera` templates.

#[cfg(test)]
use crate::server::{Document, Documents};
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    syntax,
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{Position, Range, RenameParams, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;

/// Handles a `textDocument/rename` request.
#[allow(clippy::mutable_key_type)]
pub fn handle(state: &ServerState, params: RenameParams) -> anyhow::Result<Option<WorkspaceEdit>> {
    let text_document_position = params.text_document_position;
    let uri = text_document_position.text_document.uri;
    let position = text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => achitekfile_rename(state, uri, position, &params.new_name),
        DocumentKind::TeraTemplate => tera_rename(state, uri, position, &params.new_name),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

#[allow(clippy::mutable_key_type)]
fn achitekfile_rename(
    state: &ServerState,
    uri: Uri,
    position: Position,
    new_name: &str,
) -> anyhow::Result<Option<WorkspaceEdit>> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(None);
    };

    let analysis = editor::build(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let cursor_position = to_text_position(position);
    let Some(prompt_name) = analysis.prompt_name(cursor_position).map(str::to_owned) else {
        return Ok(None);
    };
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    add_achitekfile_edits(
        &mut changes,
        uri.clone(),
        &document.text,
        &analysis,
        cursor_position,
        new_name,
    );

    if let Some(project) = ProjectContext::for_uri(state, &uri) {
        add_template_edits(&mut changes, &project, &prompt_name, new_name)?;
    }

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

#[allow(clippy::mutable_key_type)]
fn tera_rename(
    state: &ServerState,
    uri: Uri,
    position: Position,
    new_name: &str,
) -> anyhow::Result<Option<WorkspaceEdit>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template rename skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(?uri, "template rename skipped because no project was found");
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;
    let Some(prompt_name) = utils::reference_at_position(&source, position) else {
        tracing::debug!(?uri, ?position, "no template reference under cursor");
        return Ok(None);
    };

    let achitek_uri = project.achitekfile_uri()?;
    let achitek_source = project.achitekfile_source()?;
    let analysis = editor::build(&achitek_source).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let Some(symbol) = analysis
        .symbols()
        .iter()
        .find(|symbol| symbol.kind() == editor::SymbolKind::Prompt && symbol.name() == prompt_name)
    else {
        tracing::debug!(
            ?uri,
            reference = prompt_name,
            target = ?achitek_uri,
            "template rename skipped because prompt was not found"
        );
        return Ok(None);
    };

    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    add_achitekfile_edits(
        &mut changes,
        achitek_uri,
        &achitek_source,
        &analysis,
        symbol.selection_range().start_position,
        new_name,
    );

    add_template_edits(&mut changes, &project, &prompt_name, new_name)?;

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

#[allow(clippy::mutable_key_type)]
fn add_achitekfile_edits(
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    uri: Uri,
    source: &str,
    analysis: &editor::DocumentModel,
    position: syntax::TextPosition,
    new_name: &str,
) {
    for target in analysis.references(position, true) {
        let range = to_lsp_range(target.range());
        let replacement = replacement_text_for_range(source, &range, new_name);
        changes.entry(uri.clone()).or_default().push(TextEdit {
            range,
            new_text: replacement,
        });
    }
}

#[allow(clippy::mutable_key_type)]
fn add_template_edits(
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
    project: &ProjectContext<'_>,
    prompt_name: &str,
    new_name: &str,
) -> anyhow::Result<()> {
    for location in project.scan_template_references(prompt_name)? {
        let Some(path) = utils::file_path_from_uri(&location.uri) else {
            continue;
        };
        let source = project.template_source(&location.uri, &path)?;
        let replacement = replacement_text_for_range(&source, &location.range, new_name);
        changes.entry(location.uri).or_default().push(TextEdit {
            range: location.range,
            new_text: replacement,
        });
    }

    Ok(())
}

fn replacement_text_for_range(source: &str, range: &Range, new_name: &str) -> String {
    if selected_text(source, range).is_some_and(|text| text.starts_with('"') && text.ends_with('"'))
    {
        format!("\"{new_name}\"")
    } else {
        new_name.to_owned()
    }
}

fn selected_text<'a>(source: &'a str, range: &Range) -> Option<&'a str> {
    if range.start.line != range.end.line {
        return None;
    }

    let line = source
        .lines()
        .nth(usize::try_from(range.start.line).ok()?)?;
    let start = usize::try_from(range.start.character).ok()?;
    let end = usize::try_from(range.end.character).ok()?;
    line.get(start..end)
}

fn to_text_position(position: Position) -> syntax::TextPosition {
    syntax::TextPosition {
        row: usize::try_from(position.line).expect("line should fit into usize"),
        column: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

fn to_lsp_range(range: syntax::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start_position),
        end: to_lsp_position(range.end_position),
    }
}

fn to_lsp_position(position: syntax::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.row).expect("line should fit into u32"),
        character: u32::try_from(position.column).expect("column should fit into u32"),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;
    use lsp_server::{Connection, Message, Request, RequestId, Response};
    use lsp_types::{
        TextDocumentIdentifier, TextDocumentPositionParams,
        request::{Rename, Request as LspRequest},
    };
    use std::fs;

    fn handle(
        connection: &Connection,
        request: &Request,
        documents: &Documents,
    ) -> anyhow::Result<()> {
        let params = serde_json::from_value(request.params.clone())?;
        let state = ServerState {
            documents: documents.clone(),
            ..Default::default()
        };
        let result = super::handle(&state, params)?;
        connection.sender.send(Message::Response(Response::new_ok(
            request.id.clone(),
            result,
        )))?;
        Ok(())
    }

    #[test]
    #[allow(clippy::mutable_key_type)]
    fn handle_rename_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
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
        let documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: reference_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let edit: Option<WorkspaceEdit> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let changes = edit
            .expect("workspace edit should be available")
            .changes
            .expect("workspace edit should contain changes");
        let edits = changes.get(&uri).expect("uri should have edits");
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().any(|edit| edit.new_text == "\"repository\""));
        assert!(edits.iter().any(|edit| edit.new_text == "repository"));

        Ok(())
    }

    #[test]
    #[allow(clippy::mutable_key_type)]
    fn handle_rename_request_includes_template_edits() -> anyhow::Result<()> {
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
        let uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
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
        let documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: reference_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let edit: Option<WorkspaceEdit> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let changes = edit
            .expect("workspace edit should be available")
            .changes
            .expect("workspace edit should contain changes");
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
    fn handle_template_rename_request_uses_workspace_project() -> anyhow::Result<()> {
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
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
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
            workspace: crate::workspace::Workspace::discover(&temp_root)?,
            ..Default::default()
        };

        let edit = super::handle(
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

    fn recv_response(connection: &Connection) -> anyhow::Result<Response> {
        match connection.receiver.recv()? {
            Message::Response(response) => Ok(response),
            message => anyhow::bail!("expected response, got {message:?}"),
        }
    }

    fn test_uri() -> anyhow::Result<Uri> {
        Ok("file:///workspace/Achitekfile".parse()?)
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
}
