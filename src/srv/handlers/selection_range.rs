//! Handler for the LSP `textDocument/selectionRange` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_selectionRange>
//!
//! Clients send this request to expand a cursor position into progressively
//! larger source ranges. Editors use the response for "expand selection" and
//! similar structural-selection commands.
//!
//! For Achitekfiles, selection ranges are built from analyzed symbols. A cursor
//! inside a prompt name can expand from the prompt name, to the whole prompt
//! block, and then to larger containing symbols when available.

#[cfg(test)]
use crate::server::Document;
use crate::{analysis, server::Documents, syntax};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
#[cfg(test)]
use lsp_types::Uri;
use lsp_types::{Position, Range, SelectionRange, SelectionRangeParams};

/// Handles a `textDocument/selectionRange` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler analyzes its text and returns a selection
/// range chain for each requested position that falls inside a known symbol. If
/// the document is unknown, the handler returns `null`.
pub fn handle(
    connection: &Connection,
    request: &Request,
    documents: &Documents,
) -> anyhow::Result<()> {
    let params: SelectionRangeParams = serde_json::from_value(request.params.clone())
        .context("failed to parse selectionRange params")?;

    let result = if let Some(document) = documents.get(params.text_document.uri.as_str()) {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;

        Some(
            params
                .positions
                .iter()
                .filter_map(|position| selection_range_for_position(&analysis, *position))
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    let response = Response::new_ok(request.id.clone(), result);
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send selectionRange response")?;

    Ok(())
}

/// Builds the nested LSP selection range for a single position.
fn selection_range_for_position(
    analysis: &analysis::Analysis,
    position: Position,
) -> Option<SelectionRange> {
    let position = syntax::TextPosition {
        row: usize::try_from(position.line).ok()?,
        column: usize::try_from(position.character).ok()?,
    };
    let mut candidates = Vec::new();

    for symbol in analysis.symbols() {
        collect_selection_candidates(symbol, position, &mut candidates);
    }

    candidates.sort_by_key(|range| {
        (
            range
                .end_position
                .row
                .saturating_sub(range.start_position.row),
            range
                .end_position
                .column
                .saturating_sub(range.start_position.column),
        )
    });

    let mut current = None;
    for range in candidates.into_iter().rev() {
        current = Some(SelectionRange {
            range: to_lsp_range(range),
            parent: current.map(Box::new),
        });
    }

    current
}

/// Collects symbol ranges that contain the requested position.
fn collect_selection_candidates(
    symbol: &analysis::Symbol,
    position: syntax::TextPosition,
    candidates: &mut Vec<syntax::TextRange>,
) {
    if contains_position(symbol.selection_range(), position) {
        candidates.push(symbol.selection_range());
    }
    if contains_position(symbol.range(), position) {
        candidates.push(symbol.range());
    }

    for child in symbol.children() {
        collect_selection_candidates(child, position, candidates);
    }
}

/// Returns true when a position is inside a source range.
fn contains_position(range: syntax::TextRange, position: syntax::TextPosition) -> bool {
    (position.row > range.start_position.row
        || (position.row == range.start_position.row
            && position.column >= range.start_position.column))
        && (position.row < range.end_position.row
            || (position.row == range.end_position.row
                && position.column <= range.end_position.column))
}

/// Converts an analysis text range into an LSP range.
fn to_lsp_range(range: syntax::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start_position),
        end: to_lsp_position(range.end_position),
    }
}

/// Converts a zero-based analysis text position into an LSP position.
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
        request::{Request as LspRequest, SelectionRangeRequest},
    };

    #[test]
    fn handle_selection_range_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = selection_range_request(
            request_id.clone(),
            uri.clone(),
            vec![Position {
                line: 5,
                character: 10,
            }],
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

        let ranges: Option<Vec<SelectionRange>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let ranges = ranges.expect("selection ranges should be available");

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].range.start.line, 5);
        assert_eq!(ranges[0].range.start.character, 7);
        assert_eq!(ranges[0].range.end.line, 5);
        assert_eq!(ranges[0].range.end.character, 21);

        let parent = ranges[0]
            .parent
            .as_ref()
            .expect("selection range should have a parent");
        assert_eq!(parent.range.start.line, 5);
        assert_eq!(parent.range.start.character, 0);
        assert_eq!(parent.range.end.line, 8);

        Ok(())
    }

    #[test]
    fn handle_unknown_document_selection_range_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = selection_range_request(
            request_id.clone(),
            test_uri()?,
            vec![Position {
                line: 5,
                character: 10,
            }],
        );
        let documents = Documents::new();

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let ranges: Option<Vec<SelectionRange>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        assert!(ranges.is_none());

        Ok(())
    }

    fn selection_range_request(id: RequestId, uri: Uri, positions: Vec<Position>) -> Request {
        Request::new(
            id,
            SelectionRangeRequest::METHOD.to_owned(),
            SelectionRangeParams {
                text_document: TextDocumentIdentifier { uri },
                positions,
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
            }
        "#}
        .to_owned()
    }
}
