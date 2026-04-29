//! Handler for the LSP `textDocument/rename` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_rename>
//!
//! Clients send this request after the user confirms a new symbol name. For
//! Achitekfiles, this returns a workspace edit that renames a prompt declaration,
//! its document-local references, and matching prompt references in nearby
//! `.tera` templates.

use crate::{
    analysis,
    server::{Document, utils},
    syntax,
};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{Position, Range, RenameParams, TextEdit, Uri, WorkspaceEdit};
use std::{collections::HashMap, fs};

/// Handles a `textDocument/rename` request.
#[allow(clippy::mutable_key_type)]
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: RenameParams =
        serde_json::from_value(request.params.clone()).context("failed to parse rename params")?;
    let text_document_position = params.text_document_position;

    let result = if let Some(document) = documents.get(&text_document_position.text_document.uri) {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                text_document_position.text_document.uri
            )
        })?;
        let cursor_position = to_text_position(text_document_position.position);
        let Some(prompt_name) = analysis.prompt_name(cursor_position).map(str::to_owned) else {
            return send_response(connection, request, None);
        };
        let references = analysis.references(cursor_position, true);
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();

        for target in references {
            let range = to_lsp_range(target.range());
            let replacement = replacement_text_for_range(&document.text, &range, &params.new_name);
            changes
                .entry(text_document_position.text_document.uri.clone())
                .or_default()
                .push(TextEdit {
                    range,
                    new_text: replacement,
                });
        }

        if let Some(blueprint_dir) =
            utils::blueprint_dir_from_uri(&text_document_position.text_document.uri)
        {
            for location in utils::scan_references(&blueprint_dir, &prompt_name)? {
                let Some(path) = utils::file_path_from_uri(&location.uri) else {
                    continue;
                };
                let source = fs::read_to_string(&path).with_context(|| {
                    format!("failed to read template for rename `{}`", path.display())
                })?;
                let replacement =
                    replacement_text_for_range(&source, &location.range, &params.new_name);
                changes.entry(location.uri).or_default().push(TextEdit {
                    range: location.range,
                    new_text: replacement,
                });
            }
        }

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    } else {
        None
    };

    send_response(connection, request, result)
}

fn send_response(
    connection: &Connection,
    request: &Request,
    result: Option<WorkspaceEdit>,
) -> anyhow::Result<()> {
    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send rename response")?;

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
    use lsp_server::RequestId;
    use lsp_types::{
        TextDocumentIdentifier, TextDocumentPositionParams,
        request::{Rename, Request as LspRequest},
    };

    #[test]
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
        let documents = HashMap::from([(
            uri.clone(),
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
        let documents = HashMap::from([(
            uri.clone(),
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
