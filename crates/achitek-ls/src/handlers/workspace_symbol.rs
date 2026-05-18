//! Handler for the LSP `workspace/symbol` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol>
//!
//! Clients send this request when the user searches for symbols across the
//! workspace. This handler searches the server's in-memory Achitek documents
//! and returns prompt symbols whose names match the query.

#[cfg(test)]
use crate::server::Document;
use crate::{editor, server::Documents, syntax};
use anyhow::Context;
use lsp_types::{
    Location, Position, Range, SymbolInformation, SymbolKind as LspSymbolKind, Uri,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
};

/// Handles a `workspace/symbol` request.
pub fn handle(
    documents: &Documents,
    params: WorkspaceSymbolParams,
) -> anyhow::Result<Option<WorkspaceSymbolResponse>> {
    let query = params.query.to_lowercase();
    let mut symbols = Vec::new();

    for (uri, document) in documents {
        let uri = uri
            .parse::<Uri>()
            .with_context(|| format!("failed to parse document URI `{uri}`"))?;
        let analysis = editor::build(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;

        for symbol in analysis.symbols() {
            if symbol.kind() != editor::SymbolKind::Prompt {
                continue;
            }
            if !query.is_empty() && !symbol.name().to_lowercase().contains(&query) {
                continue;
            }

            symbols.push(to_lsp_symbol_information(&uri, symbol));
        }
    }

    Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
}

#[allow(deprecated)]
fn to_lsp_symbol_information(uri: &Uri, symbol: &editor::Symbol) -> SymbolInformation {
    SymbolInformation {
        name: symbol.name().to_owned(),
        kind: match symbol.kind() {
            editor::SymbolKind::Blueprint => LspSymbolKind::MODULE,
            editor::SymbolKind::Prompt => LspSymbolKind::FIELD,
            editor::SymbolKind::Validate => LspSymbolKind::OBJECT,
        },
        tags: None,
        deprecated: None,
        location: Location::new(uri.clone(), to_lsp_range(symbol.selection_range())),
        container_name: Some("Achitekfile".to_owned()),
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
    use lsp_server::{Connection, Message, Request, RequestId, Response};
    use lsp_types::request::{Request as LspRequest, WorkspaceSymbolRequest};

    fn handle(
        connection: &Connection,
        request: &Request,
        documents: &Documents,
    ) -> anyhow::Result<()> {
        let params = serde_json::from_value(request.params.clone())?;
        let result = super::handle(documents, params)?;
        connection.sender.send(Message::Response(Response::new_ok(
            request.id.clone(),
            result,
        )))?;
        Ok(())
    }

    #[test]
    fn handle_workspace_symbol_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            WorkspaceSymbolRequest::METHOD.to_owned(),
            WorkspaceSymbolParams {
                query: "project".to_owned(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        );
        let documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let result: Option<WorkspaceSymbolResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(WorkspaceSymbolResponse::Flat(symbols)) = result else {
            panic!("expected flat workspace symbols");
        };
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "project_name");

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
