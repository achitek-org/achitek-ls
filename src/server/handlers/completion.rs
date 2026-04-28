//! Handler for the LSP `textDocument/completion` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_completion>
//!
//! Clients send this request when they need completion items at a cursor
//! position. For Achitekfiles, completions include DSL keywords, attributes,
//! prompt types, references, and dependency-expression helpers.

use crate::{analysis, server::Document, syntax};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Position, Uri,
};
use std::collections::HashMap;

/// Handles a `textDocument/completion` request.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: CompletionParams = serde_json::from_value(request.params.clone())
        .context("failed to parse completion params")?;

    let result =
        if let Some(document) = documents.get(&params.text_document_position.text_document.uri) {
            let analysis = analysis::analyze(&document.text).with_context(|| {
                format!(
                    "failed to analyze document `{:?}`",
                    params.text_document_position.text_document.uri
                )
            })?;
            let items = analysis
                .completions(to_text_position(params.text_document_position.position))
                .into_iter()
                .map(to_lsp_completion_item)
                .collect::<Vec<_>>();

            Some(CompletionResponse::Array(items))
        } else {
            None
        };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send completion response")?;

    Ok(())
}

/// Converts an analysis completion into an LSP completion item.
fn to_lsp_completion_item(item: analysis::Completion) -> CompletionItem {
    CompletionItem {
        label: item.label().to_owned(),
        detail: item.detail().map(str::to_owned),
        kind: Some(match item.kind() {
            analysis::CompletionKind::Keyword => CompletionItemKind::KEYWORD,
            analysis::CompletionKind::Property => CompletionItemKind::PROPERTY,
            analysis::CompletionKind::Value => CompletionItemKind::VALUE,
            analysis::CompletionKind::Reference => CompletionItemKind::REFERENCE,
            analysis::CompletionKind::Function => CompletionItemKind::FUNCTION,
        }),
        ..CompletionItem::default()
    }
}

/// Converts an LSP position into an analysis position.
fn to_text_position(position: Position) -> syntax::TextPosition {
    syntax::TextPosition {
        row: usize::try_from(position.line).expect("line should fit into usize"),
        column: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;
    use lsp_server::RequestId;
    use lsp_types::{
        TextDocumentIdentifier, TextDocumentPositionParams,
        request::{Completion, Request as LspRequest},
    };

    #[test]
    fn handle_completion_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            Completion::METHOD.to_owned(),
            CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 6,
                        character: 2,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        );
        let documents = HashMap::from([(
            uri,
            Document {
                version: 1,
                text: valid_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let result: Option<CompletionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(CompletionResponse::Array(items)) = result else {
            panic!("expected completion item array");
        };
        assert!(items.iter().any(|item| item.label == "type"));

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
              
            }
        "#}
        .to_owned()
    }
}
