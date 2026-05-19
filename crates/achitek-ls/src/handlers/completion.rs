//! Handler for the LSP `textDocument/completion` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_completion>
//!
//! Clients send this request when they need completion items at a cursor
//! position. For Achitekfiles, completions include DSL keywords, attributes,
//! prompt types, references, and dependency-expression helpers.

#[cfg(test)]
use crate::server::{Document, Documents};
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Position, Uri,
};

/// Handles a `textDocument/completion` request.
pub fn handle(
    state: &ServerState,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => achitekfile_completions(state, uri, position),
        DocumentKind::TeraTemplate => tera_completions(state, uri, position),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_completions(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<CompletionResponse>> {
    if let Some(document) = state.documents.get(uri.as_str()) {
        let editor_buffer = editor::from_source(&document.text)
            .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
        let items = editor_buffer
            .completions(to_text_position(position))
            .into_iter()
            .map(to_lsp_completion_item)
            .collect::<Vec<_>>();

        Ok(Some(CompletionResponse::Array(items)))
    } else {
        Ok(None)
    }
}

fn tera_completions(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<CompletionResponse>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template completion skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template completion skipped because no project was found"
        );
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;
    if !utils::is_template_expression_position(&source, position) {
        return Ok(None);
    }

    let achitek_source = project.achitekfile_source()?;
    let editor_buffer = editor::from_source(&achitek_source).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let items = editor_buffer
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == editor::SymbolKind::Prompt)
        .map(|symbol| CompletionItem {
            label: symbol.name().to_owned(),
            detail: Some("Prompt reference".to_owned()),
            kind: Some(CompletionItemKind::REFERENCE),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    Ok(Some(CompletionResponse::Array(items)))
}

/// Converts an editor completion into an LSP completion item.
fn to_lsp_completion_item(item: editor::Completion) -> CompletionItem {
    CompletionItem {
        label: item.label().to_owned(),
        detail: item.detail().map(str::to_owned),
        kind: Some(match item.kind() {
            editor::CompletionKind::Keyword => CompletionItemKind::KEYWORD,
            editor::CompletionKind::Property => CompletionItemKind::PROPERTY,
            editor::CompletionKind::Value => CompletionItemKind::VALUE,
            editor::CompletionKind::Reference => CompletionItemKind::REFERENCE,
            editor::CompletionKind::Function => CompletionItemKind::FUNCTION,
        }),
        ..CompletionItem::default()
    }
}

/// Converts an LSP position into an editor position.
fn to_text_position(position: Position) -> achitekfile::TextPosition {
    achitekfile::TextPosition {
        line: usize::try_from(position.line).expect("line should fit into usize"),
        byte: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::utils;
    use indoc::indoc;
    use lsp_server::{Connection, Message, Request, RequestId, Response};
    use lsp_types::{
        TextDocumentIdentifier, TextDocumentPositionParams,
        request::{Completion, Request as LspRequest},
    };
    use std::fs;

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

        let result: Option<CompletionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(CompletionResponse::Array(items)) = result else {
            panic!("expected completion item array");
        };
        assert!(items.iter().any(|item| item.label == "type"));

        Ok(())
    }

    #[test]
    fn handle_template_completion_request_returns_prompt_names() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-completion")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, valid_source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        let template_source = "{{ pro".to_owned();
        fs::write(&template_path, &template_source)?;
        let uri = utils::path_to_uri(&template_path)?;
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 0,
                    character: 6,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };
        let state = ServerState {
            documents: Documents::from([(
                uri.as_str().to_owned(),
                Document {
                    version: 1,
                    text: template_source,
                },
            )]),
            ..Default::default()
        };

        let Some(CompletionResponse::Array(items)) = super::handle(&state, params)? else {
            panic!("expected completion item array");
        };

        assert!(items.iter().any(|item| item.label == "project_name"));

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
              
            }
        "#}
        .to_owned()
    }
}
