//! Handler for the LSP `textDocument/foldingRange` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_foldingRange>
//!
//! Clients send this request when they need the foldable regions for a single
//! document. Editors use the response to draw folding controls in the gutter
//! and to support commands such as folding the current block or all foldable
//! blocks in a file.
//!
//! For Achitekfiles, folding ranges are derived from analyzed document symbols.
//! Multi-line `blueprint`, `prompt`, and nested `validate` symbols become
//! foldable ranges, with the symbol name used as collapsed text when supported
//! by the client.

#[cfg(test)]
use crate::server::Document;
use crate::{analysis, server::Documents};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
#[cfg(test)]
use lsp_types::Uri;
use lsp_types::{FoldingRange, FoldingRangeParams};

/// Handles a `textDocument/foldingRange` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler analyzes its text and returns foldable ranges
/// for multi-line Achitek symbols. If the document is unknown, the handler
/// returns `null`.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &Documents,
) -> anyhow::Result<()> {
    let params: FoldingRangeParams = serde_json::from_value(request.params.clone())
        .context("failed to parse foldingRange params")?;

    let result = if let Some(document) = documents.get(params.text_document.uri.as_str()) {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;
        let mut ranges = Vec::new();

        for symbol in analysis.symbols() {
            collect_folding_ranges(symbol, &mut ranges);
        }

        Some(ranges)
    } else {
        None
    };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send foldingRange response")?;

    Ok(())
}

/// Collects foldable ranges from an analysis symbol and its children.
fn collect_folding_ranges(symbol: &analysis::Symbol, ranges: &mut Vec<FoldingRange>) {
    let range = symbol.range();

    if range.start_position.row < range.end_position.row {
        ranges.push(FoldingRange {
            start_line: u32::try_from(range.start_position.row).expect("line should fit into u32"),
            start_character: Some(
                u32::try_from(range.start_position.column).expect("column should fit into u32"),
            ),
            end_line: u32::try_from(range.end_position.row).expect("line should fit into u32"),
            end_character: Some(
                u32::try_from(range.end_position.column).expect("column should fit into u32"),
            ),
            kind: None,
            collapsed_text: Some(symbol.name().to_owned()),
        });
    }

    for child in symbol.children() {
        collect_folding_ranges(child, ranges);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;
    use lsp_server::RequestId;
    use lsp_types::{
        TextDocumentIdentifier,
        request::{FoldingRangeRequest, Request as LspRequest},
    };

    #[test]
    fn handle_folding_range_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = folding_range_request(request_id.clone(), uri.clone());
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

        let ranges: Option<Vec<FoldingRange>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let ranges = ranges.expect("folding ranges should be available");

        assert!(
            ranges.iter().any(|range| range.start_line == 0
                && range.collapsed_text.as_deref() == Some("blueprint"))
        );
        assert!(ranges.iter().any(|range| range.start_line == 5
            && range.collapsed_text.as_deref() == Some("project_name")));
        assert!(
            ranges.iter().any(|range| range.start_line == 8
                && range.collapsed_text.as_deref() == Some("validate"))
        );

        Ok(())
    }

    #[test]
    fn handle_unknown_document_folding_range_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = folding_range_request(request_id.clone(), test_uri()?);
        let documents = Documents::new();

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let ranges: Option<Vec<FoldingRange>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        assert!(ranges.is_none());

        Ok(())
    }

    fn folding_range_request(id: RequestId, uri: Uri) -> Request {
        Request::new(
            id,
            FoldingRangeRequest::METHOD.to_owned(),
            FoldingRangeParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
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

    fn valid_source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
              validate {
                min_length = 2
              }
            }
        "#}
        .to_owned()
    }
}
