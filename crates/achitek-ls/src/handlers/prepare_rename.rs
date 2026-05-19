//! Handler for the LSP `textDocument/prepareRename` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_prepareRename>
//!
//! Clients send this request before showing rename UI. The response tells the
//! client whether the cursor is on a renameable symbol and which range should
//! be edited.

#[cfg(test)]
use crate::server::{Document, Documents};
use crate::{
    editor,
    server::{ServerState, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
#[cfg(test)]
use lsp_types::Uri;
use lsp_types::{Position, PrepareRenameResponse, Range, TextDocumentPositionParams};
use std::fs;

/// Handles a `textDocument/prepareRename` request.
pub fn handle(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    match state.document_kind(&params.text_document.uri) {
        DocumentKind::Achitekfile => achitekfile_prepare_rename(state, params),
        DocumentKind::TeraTemplate => tera_prepare_rename(state, params),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_prepare_rename(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    let Some(document) = state.documents.get(params.text_document.uri.as_str()) else {
        return Ok(None);
    };

    let editor_buffer = editor::from_source(&document.text).with_context(|| {
        format!(
            "failed to analyze document `{:?}`",
            params.text_document.uri
        )
    })?;
    Ok(editor_buffer
        .prepare_rename(to_text_position(params.position))
        .map(|target| PrepareRenameResponse::RangeWithPlaceholder {
            range: to_lsp_range(target.range()),
            placeholder: target.placeholder().to_owned(),
        }))
}

fn tera_prepare_rename(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> anyhow::Result<Option<PrepareRenameResponse>> {
    let uri = params.text_document.uri;
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template prepare rename skipped for non-file URI");
        return Ok(None);
    };
    let source = state
        .documents
        .get(uri.as_str())
        .map(|document| Ok(document.text.clone()))
        .unwrap_or_else(|| {
            fs::read_to_string(&template_path)
                .with_context(|| format!("failed to read template `{}`", template_path.display()))
        })?;

    Ok(
        utils::reference_target_at_position(&source, params.position).map(
            |(placeholder, range)| PrepareRenameResponse::RangeWithPlaceholder {
                range,
                placeholder,
            },
        ),
    )
}

fn to_text_position(position: Position) -> achitekfile::TextPosition {
    achitekfile::TextPosition {
        line: usize::try_from(position.line).expect("line should fit into usize"),
        byte: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

fn to_lsp_range(range: achitekfile::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start),
        end: to_lsp_position(range.end),
    }
}

fn to_lsp_position(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use indoc::indoc;
    use lsp_server::{Connection, Message, Request, RequestId, Response};
    use lsp_types::{
        TextDocumentIdentifier,
        request::{PrepareRenameRequest, Request as LspRequest},
    };

    fn handle(
        connection: &Connection,
        request: &Request,
        documents: &Documents,
    ) -> anyhow::Result<()> {
        let params = serde_json::from_value(request.params.clone())?;
        let state = ServerState {
            documents: documents.clone(),
            ..Default::default()
        };
        let result = super::handle(&state, params)?;
        connection.sender.send(Message::Response(Response::new_ok(
            request.id.clone(),
            result,
        )))?;
        Ok(())
    }

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
        let documents = Documents::from([(
            uri.as_str().to_owned(),
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

    #[test]
    fn handle_template_prepare_rename_request() -> anyhow::Result<()> {
        let uri: Uri = "file:///workspace/rust/Cargo.toml.tera".parse()?;
        let state = ServerState {
            documents: Documents::from([(
                uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: indoc! {r#"[package]
                        name = "{{project_name}}"
                    "#}
                    .to_owned(),
                },
            )]),
            ..Default::default()
        };

        let result = super::handle(
            &state,
            TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 1,
                    character: 13,
                },
            },
        )?;

        let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, range }) = result
        else {
            panic!("expected range with placeholder");
        };
        assert_eq!(placeholder, "project_name");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 10);

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
