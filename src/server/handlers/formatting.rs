//! Handler for the LSP `textDocument/formatting` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_formatting>
//!
//! Clients send this request when the user asks the editor to format the whole
//! document. The server responds with text edits that the client applies to the
//! current buffer. Returning an empty edit list means the document is already
//! formatted; returning `null` means the server has no formatting result for
//! the requested document.
//!
//! For Achitekfiles, this handler currently performs a simple full-document
//! layout pass: it trims each line, applies two-space indentation for nested
//! blocks, and returns a single full-document replacement edit when the text
//! changes.
use crate::server::Document;
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{DocumentFormattingParams, Position, Range, TextEdit, Uri};
use std::collections::HashMap;

/// Handles a `textDocument/formatting` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler formats its text and returns either no edits
/// or one full-document replacement edit. If the document is unknown, the
/// handler returns `null`.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: DocumentFormattingParams = serde_json::from_value(request.params.clone())
        .context("failed to parse formatting params")?;
    let result = if let Some(document) = documents.get(&params.text_document.uri) {
        let formatted = format_achitek_source(&document.text);

        if formatted == document.text {
            Some(Vec::new())
        } else {
            Some(vec![TextEdit {
                range: full_document_range(&document.text),
                new_text: formatted,
            }])
        }
    } else {
        None
    };
    let response = Response::new_ok(request.id.clone(), result);

    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send formatting response")?;

    Ok(())
}

/// Formats Achitek source using simple brace-based indentation.
fn format_achitek_source(source: &str) -> String {
    let mut formatted = String::new();
    let mut indent = 0usize;

    for raw_line in source.lines() {
        let line = raw_line.trim();

        if line.starts_with('}') {
            indent = indent.saturating_sub(1);
        }

        if line.is_empty() {
            formatted.push('\n');
        } else {
            formatted.push_str(&"  ".repeat(indent));
            formatted.push_str(line);
            formatted.push('\n');
        }

        if line.ends_with('{') {
            indent += 1;
        }
    }

    formatted
}

/// Returns the LSP range covering the entire source document.
fn full_document_range(source: &str) -> Range {
    let last_line = source.lines().last().unwrap_or("");

    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: u32::try_from(source.lines().count()).expect("line count should fit into u32"),
            character: u32::try_from(last_line.len()).expect("line length should fit into u32"),
        },
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;
    use lsp_server::RequestId;
    use lsp_types::{
        FormattingOptions, TextDocumentIdentifier,
        request::{Formatting, Request as LspRequest},
    };

    #[test]
    fn handle_formatting_request_returns_full_document_edit() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = formatting_request(request_id.clone(), uri.clone());
        let documents = HashMap::from([(
            uri,
            Document {
                version: 1,
                text: unformatted_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let edits: Option<Vec<TextEdit>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let edits = edits.expect("formatting edits should be available");

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, formatted_source());
        assert_eq!(edits[0].range, full_document_range(&unformatted_source()));

        Ok(())
    }

    #[test]
    fn handle_formatting_request_returns_empty_edits_when_document_is_formatted()
    -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = formatting_request(request_id.clone(), uri.clone());
        let documents = HashMap::from([(
            uri,
            Document {
                version: 1,
                text: formatted_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let edits: Option<Vec<TextEdit>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        assert_eq!(edits, Some(Vec::new()));

        Ok(())
    }

    #[test]
    fn handle_unknown_document_formatting_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = formatting_request(request_id.clone(), test_uri()?);
        let documents = HashMap::new();

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let edits: Option<Vec<TextEdit>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        assert!(edits.is_none());

        Ok(())
    }

    fn formatting_request(id: RequestId, uri: Uri) -> Request {
        Request::new(
            id,
            Formatting::METHOD.to_owned(),
            DocumentFormattingParams {
                text_document: TextDocumentIdentifier { uri },
                options: FormattingOptions {
                    tab_size: 2,
                    insert_spaces: true,
                    ..FormattingOptions::default()
                },
                work_done_progress_params: Default::default(),
            },
        )
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

    fn unformatted_source() -> String {
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

    fn formatted_source() -> String {
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
