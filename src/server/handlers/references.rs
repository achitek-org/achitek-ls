//! Handler for the LSP `textDocument/references` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_references>
//!
//! Clients send this request when the user asks to find all uses of the symbol
//! at a cursor position. For Achitekfiles, this returns prompt declaration and
//! dependency-expression reference locations, plus prompt references in nearby
//! `.tera` templates.

use crate::{
    analysis,
    server::{Document, utils},
    syntax,
};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{Location, Position, Range, ReferenceParams, Uri};
use std::collections::HashMap;

/// Handles a `textDocument/references` request.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: ReferenceParams = serde_json::from_value(request.params.clone())
        .context("failed to parse references params")?;
    let text_document_position = params.text_document_position;

    let result = if let Some(document) = documents.get(&text_document_position.text_document.uri) {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                text_document_position.text_document.uri
            )
        })?;
        let cursor_position = to_text_position(text_document_position.position);
        let prompt_name = analysis.prompt_name(cursor_position).map(str::to_owned);
        let locations = analysis
            .references(cursor_position, params.context.include_declaration)
            .into_iter()
            .map(|target| {
                Location::new(
                    text_document_position.text_document.uri.clone(),
                    to_lsp_range(target.range()),
                )
            })
            .collect::<Vec<_>>();
        let mut locations = locations;

        if let (Some(prompt_name), Some(blueprint_dir)) = (
            prompt_name,
            utils::blueprint_dir_from_uri(&text_document_position.text_document.uri),
        ) {
            locations.extend(utils::scan_references(&blueprint_dir, &prompt_name)?);
        }

        Some(locations)
    } else {
        None
    };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send references response")?;

    Ok(())
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
        ReferenceContext, TextDocumentIdentifier, TextDocumentPositionParams,
        request::{References, Request as LspRequest},
    };
    use std::fs;

    #[test]
    fn handle_references_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
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
        let documents = HashMap::from([(
            uri,
            Document {
                version: 1,
                text: reference_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let locations: Option<Vec<Location>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let locations = locations.expect("references should be available");
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
    fn handle_references_request_includes_template_references() -> anyhow::Result<()> {
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
        let uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
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
        let documents = HashMap::from([(
            uri,
            Document {
                version: 1,
                text: reference_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let locations: Option<Vec<Location>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let locations = locations.expect("references should be available");
        assert_eq!(locations.len(), 3);
        assert!(
            locations
                .iter()
                .any(|location| location.uri == template_uri && location.range.start.line == 1)
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
