//! Handler for the LSP `textDocument/definition` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition>
//!
//! Clients send this request when the user asks to jump from a reference to the
//! source location that defines it. For Achitekfiles, this currently resolves
//! prompt references back to their prompt declarations. For `.tera` templates,
//! this can jump from a prompt interpolation back to the matching Achitekfile
//! prompt declaration.

#[cfg(test)]
use crate::server::Document;
use crate::{
    analysis,
    server::{Documents, utils},
    syntax,
};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
#[cfg(test)]
use lsp_types::Uri;
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range};

/// Handles a `textDocument/definition` request.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &Documents,
) -> anyhow::Result<()> {
    let params: GotoDefinitionParams = serde_json::from_value(request.params.clone())
        .context("failed to parse definition params")?;
    let text_document_position = params.text_document_position_params;

    let result =
        if let Some(document) = documents.get(text_document_position.text_document.uri.as_str()) {
            let analysis = analysis::analyze(&document.text).with_context(|| {
                format!(
                    "failed to analyze document `{:?}`",
                    text_document_position.text_document.uri
                )
            })?;

            analysis
                .definition(to_text_position(text_document_position.position))
                .map(|target| {
                    GotoDefinitionResponse::Scalar(Location::new(
                        text_document_position.text_document.uri,
                        to_lsp_range(target.selection_range()),
                    ))
                })
        } else {
            utils::definition(
                &text_document_position.text_document.uri,
                text_document_position.position,
                documents,
            )?
        };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send definition response")?;

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
        TextDocumentIdentifier, TextDocumentPositionParams,
        request::{GotoDefinition, Request as LspRequest},
    };
    use std::fs;

    #[test]
    fn handle_definition_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
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

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let result: Option<GotoDefinitionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(GotoDefinitionResponse::Scalar(location)) = result else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.range.start.line, 5);

        Ok(())
    }

    #[test]
    fn handle_template_definition_request() -> anyhow::Result<()> {
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
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
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

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let result: Option<GotoDefinitionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(GotoDefinitionResponse::Scalar(location)) = result else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.uri, achitek_uri);
        assert_eq!(location.range.start.line, 5);

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
