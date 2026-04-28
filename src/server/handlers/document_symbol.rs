//! Handler for the LSP `textDocument/documentSymbol` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentSymbol>
//!
//! Clients send this request when they need an outline of a single open
//! document. Editors commonly use the response to power outline panes,
//! breadcrumb navigation, and quick symbol pickers scoped to the current file.
//!
//! For Achitekfiles, this handler returns nested symbols for language
//! structures such as the top-level `blueprint` block, `prompt` blocks, and
//! nested `validate` blocks.
use crate::{analysis, server::Document, syntax};
use anyhow::Context;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Position, Range,
    SymbolKind as LspSymbolKind, Uri,
};
use std::collections::HashMap;

/// Handles a `textDocument/documentSymbol` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler analyzes its text and converts Achitek
/// symbols into LSP document symbols. If the document is unknown, the handler
/// returns `null`, which tells the client there are no symbols available for
/// that request.
pub fn handle(
    connection: &Connection,
    request: &Request,
    in_memory_document: &HashMap<Uri, Document>,
) -> anyhow::Result<()> {
    let params: DocumentSymbolParams = serde_json::from_value(request.params.clone())?;
    let result = if let Some(document) = in_memory_document.get(&params.text_document.uri) {
        let analysis = analysis::analyze(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                params.text_document.uri
            )
        })?;
        let symbols = analysis
            .symbols()
            .iter()
            .map(to_lsp_document_symbol)
            .collect::<Vec<_>>();

        Some(DocumentSymbolResponse::Nested(symbols))
    } else {
        None
    };
    let response = Response::new_ok(request.id.clone(), result);

    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send documentSymbol response")?;

    Ok(())
}

/// Converts an analysis symbol into the nested LSP document-symbol shape.
#[allow(deprecated)]
fn to_lsp_document_symbol(symbol: &analysis::Symbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name().to_owned(),
        detail: symbol.detail().map(str::to_owned),
        kind: match symbol.kind() {
            analysis::SymbolKind::Blueprint => LspSymbolKind::MODULE,
            analysis::SymbolKind::Prompt => LspSymbolKind::FIELD,
            analysis::SymbolKind::Validate => LspSymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        range: to_lsp_range(symbol.range()),
        selection_range: to_lsp_range(symbol.selection_range()),
        children: Some(
            symbol
                .children()
                .iter()
                .map(to_lsp_document_symbol)
                .collect(),
        ),
    }
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
        request::{DocumentSymbolRequest, Request as LspRequest},
    };

    #[test]
    fn handle_document_symbol_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            DocumentSymbolRequest::METHOD.to_owned(),
            DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
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

        let symbols: Option<DocumentSymbolResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let DocumentSymbolResponse::Nested(symbols) =
            symbols.expect("document symbols should be available")
        else {
            panic!("expected nested document symbols");
        };

        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "blueprint");
        assert_eq!(symbols[1].name, "project_name");
        assert_eq!(symbols[1].children.as_ref().map(Vec::len), Some(0));

        Ok(())
    }

    #[test]
    fn handle_unknown_document_symbol_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            DocumentSymbolRequest::METHOD.to_owned(),
            DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri: test_uri()? },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        );
        let documents = HashMap::new();

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        assert_eq!(response.id, request_id);
        assert!(response.error.is_none());

        let symbols: Option<DocumentSymbolResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        assert!(symbols.is_none());

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
