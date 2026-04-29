//! Handler for the LSP `textDocument/prepareRename` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_prepareRename>
//!
//! Clients send this request before showing rename UI. The response tells the
//! client whether the cursor is on a renameable symbol and which range should
//! be edited.

use crate::{analysis, server::Document, syntax};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{Position, PrepareRenameResponse, Range, TextDocumentPositionParams, Uri};
use std::collections::HashMap;

/// Handles a `textDocument/prepareRename` request.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: TextDocumentPositionParams = serde_json::from_value(request.params.clone())
        .context("failed to parse prepareRename params")?;

    let result = if let Some(document) = documents.get(&params.text_document.uri) {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;
        analysis
            .prepare_rename(to_text_position(params.position))
            .map(|target| PrepareRenameResponse::RangeWithPlaceholder {
                range: to_lsp_range(target.range()),
                placeholder: target.placeholder().to_owned(),
            })
    } else {
        None
    };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send prepareRename response")?;

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
        TextDocumentIdentifier,
        request::{PrepareRenameRequest, Request as LspRequest},
    };

    #[test]
    fn handle_prepare_rename_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            PrepareRenameRequest::METHOD.to_owned(),
            TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 5,
                    character: 10,
                },
            },
        );
        let documents = HashMap::from([(
            uri,
            Document {
                version: 1,
                text: source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let result: Option<PrepareRenameResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) = result else {
            panic!("expected range with placeholder");
        };
        assert_eq!(placeholder, "project_name");

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

    fn source() -> String {
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
}
