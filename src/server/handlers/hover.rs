//! Handler for the LSP `textDocument/hover` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_hover>
//!
//! Clients send this request when the user hovers over a position in the
//! document. Editors use the response to show contextual documentation near the
//! cursor.

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
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range};

/// Handles a `textDocument/hover` request.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &Documents,
) -> anyhow::Result<()> {
    let params: HoverParams =
        serde_json::from_value(request.params.clone()).context("failed to parse hover params")?;
    let text_document_position = params.text_document_position_params;

    let result = if utils::is_template_uri(&text_document_position.text_document.uri) {
        utils::hover(
            &text_document_position.text_document.uri,
            text_document_position.position,
            documents,
        )?
    } else if let Some(document) = documents.get(text_document_position.text_document.uri.as_str())
    {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                text_document_position.text_document.uri
            )
        })?;
        analysis
            .hover(to_text_position(text_document_position.position))
            .map(to_lsp_hover)
    } else {
        None
    };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send hover response")?;

    Ok(())
}

/// Converts analysis hover content into an LSP hover response.
fn to_lsp_hover(hover: analysis::Hover) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents().to_owned(),
        }),
        range: Some(to_lsp_range(hover.range())),
    }
}

/// Converts an LSP position into an analysis position.
fn to_text_position(position: Position) -> syntax::TextPosition {
    syntax::TextPosition {
        row: usize::try_from(position.line).expect("line should fit into usize"),
        column: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

/// Converts an analysis text range into an LSP range.
fn to_lsp_range(range: syntax::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start_position),
        end: to_lsp_position(range.end_position),
    }
}

/// Converts an analysis text position into an LSP position.
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
        request::{HoverRequest, Request as LspRequest},
    };
    use std::fs;

    #[test]
    fn handle_hover_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            HoverRequest::METHOD.to_owned(),
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 6,
                        character: 9,
                    },
                },
                work_done_progress_params: Default::default(),
            },
        );
        let documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: valid_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let hover: Option<Hover> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let hover = hover.expect("hover should be available");
        let HoverContents::Markup(contents) = hover.contents else {
            panic!("expected markup hover contents");
        };
        assert!(contents.value.contains("string"));

        Ok(())
    }

    #[test]
    fn handle_unknown_document_hover_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            HoverRequest::METHOD.to_owned(),
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: test_uri()? },
                    position: Position {
                        line: 6,
                        character: 9,
                    },
                },
                work_done_progress_params: Default::default(),
            },
        );

        handle(&server_connection, &request, &Documents::new())?;

        let response = recv_response(&client_connection)?;
        let hover: Option<Hover> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        assert!(hover.is_none());

        Ok(())
    }

    #[test]
    fn handle_template_hover_request_for_known_prompt() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-hover-known")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, valid_source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, "name = \"{{project_name}}\"")?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
            HoverRequest::METHOD.to_owned(),
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: template_uri.clone(),
                    },
                    position: Position {
                        line: 0,
                        character: 12,
                    },
                },
                work_done_progress_params: Default::default(),
            },
        );
        let documents = Documents::from([
            (
                achitek_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: valid_source(),
                },
            ),
            (
                template_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: "name = \"{{project_name}}\"".to_owned(),
                },
            ),
        ]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let hover: Option<Hover> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let hover = hover.expect("hover should be available");
        let HoverContents::Markup(contents) = hover.contents else {
            panic!("expected markup hover contents");
        };
        assert!(contents.value.contains("Achitek prompt reference"));
        assert!(contents.value.contains("project_name"));
        assert!(contents.value.contains("Kind: `prompt`"));

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn handle_template_hover_request_for_unknown_prompt() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-hover-unknown")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, valid_source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, "name = \"{{missing_prompt}}\"")?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
            HoverRequest::METHOD.to_owned(),
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: template_uri.clone(),
                    },
                    position: Position {
                        line: 0,
                        character: 12,
                    },
                },
                work_done_progress_params: Default::default(),
            },
        );
        let documents = Documents::from([
            (
                achitek_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: valid_source(),
                },
            ),
            (
                template_uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: "name = \"{{missing_prompt}}\"".to_owned(),
                },
            ),
        ]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let hover: Option<Hover> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let hover = hover.expect("hover should be available");
        let HoverContents::Markup(contents) = hover.contents else {
            panic!("expected markup hover contents");
        };
        assert!(contents.value.contains("Unknown Achitek prompt reference"));
        assert!(contents.value.contains("missing_prompt"));

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

    fn valid_source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }
        "#}
        .to_owned()
    }
}
