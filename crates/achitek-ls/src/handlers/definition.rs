//! Handler for the LSP `textDocument/definition` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition>
//!
//! Clients send this request when the user asks to jump from a reference to the
//! source location that defines it. For Achitekfiles, this currently resolves
//! prompt references back to their prompt declarations. For `.tera` templates,
//! this can jump from a prompt interpolation back to the matching Achitekfile
//! prompt declaration.

#[cfg(test)]
use crate::server::{Document, Documents};
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, Uri};

/// Handles a `textDocument/definition` request.
pub fn handle(
    state: &ServerState,
    params: GotoDefinitionParams,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let text_document_position = params.text_document_position_params;
    let uri = text_document_position.text_document.uri;
    let position = text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => achitekfile_definition(state, uri, position),
        DocumentKind::TeraTemplate => tera_definition(state, uri, position),
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_definition(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(None);
    };

    let editor_buffer = editor::from_source(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;

    Ok(editor_buffer
        .definition(to_text_position(position))
        .map(|target| {
            GotoDefinitionResponse::Scalar(Location::new(
                uri,
                to_lsp_range(target.selection_range()),
            ))
        }))
}

fn tera_definition(
    state: &ServerState,
    uri: Uri,
    position: Position,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template definition skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template definition skipped because no project was found"
        );
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;

    let Some(reference_name) = utils::reference_at_position(&source, position) else {
        tracing::debug!(?uri, ?position, "no template reference under cursor");
        return Ok(None);
    };

    let achitek_uri = project.achitekfile_uri()?;
    let achitek_source = project.achitekfile_source()?;
    let editor_buffer = editor::from_source(&achitek_source).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let Some(symbol) = editor_buffer.symbols().iter().find(|symbol| {
        symbol.kind() == editor::SymbolKind::Prompt && symbol.name() == reference_name
    }) else {
        tracing::debug!(
            ?uri,
            reference = reference_name,
            target = ?achitek_uri,
            "template definition skipped because prompt was not found"
        );
        return Ok(None);
    };

    tracing::debug!(
        ?uri,
        reference = reference_name,
        target = ?achitek_uri,
        "resolved template definition"
    );

    Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
        achitek_uri,
        to_lsp_range(symbol.selection_range()),
    ))))
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
        TextDocumentIdentifier, TextDocumentPositionParams,
        request::{GotoDefinition, Request as LspRequest},
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
    fn handle_definition_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            GotoDefinition::METHOD.to_owned(),
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 13,
                        character: 16,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        );
        let documents = Documents::from([(
            uri.as_str().to_owned(),
            Document {
                version: 1,
                text: reference_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let result: Option<GotoDefinitionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(GotoDefinitionResponse::Scalar(location)) = result else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.range.start.line, 5);

        Ok(())
    }

    #[test]
    fn handle_template_definition_request() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-definition")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, reference_source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"[package]
                name = "{{project_name}}"
            "#},
        )?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
            GotoDefinition::METHOD.to_owned(),
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: template_uri },
                    position: Position {
                        line: 1,
                        character: 13,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        );
        let documents = Documents::from([(
            achitek_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: reference_source(),
            },
        )]);

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let result: Option<GotoDefinitionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let Some(GotoDefinitionResponse::Scalar(location)) = result else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.uri, achitek_uri);
        assert_eq!(location.range.start.line, 5);

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn handle_open_template_definition_uses_workspace_project() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-workspace-template-definition")?;
        let project_root = temp_root.join("rust");
        fs::create_dir_all(&project_root)?;
        fs::write(
            temp_root.join("blueprints.toml"),
            indoc! {r#"
                [rust]
                path = "./rust"
            "#},
        )?;
        let achitek_path = project_root.join("achitekfile");
        fs::write(&achitek_path, reference_source())?;
        let template_path = project_root.join("Cargo.toml.tera");
        fs::write(&template_path, "")?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let state = ServerState {
            documents: Documents::from([
                (
                    achitek_uri.as_str().to_owned(),
                    Document {
                        version: 1,
                        text: reference_source(),
                    },
                ),
                (
                    template_uri.as_str().to_owned(),
                    Document {
                        version: 1,
                        text: indoc! {r#"[package]
                            name = "{{project_name}}"
                        "#}
                        .to_owned(),
                    },
                ),
            ]),
            workspace: crate::workspace::Workspace::discover(&temp_root)?,
            ..Default::default()
        };

        let result = super::handle(
            &state,
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: template_uri },
                    position: Position {
                        line: 1,
                        character: 13,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )?;

        let Some(GotoDefinitionResponse::Scalar(location)) = result else {
            panic!("expected scalar definition response");
        };
        assert_eq!(location.uri, achitek_uri);
        assert_eq!(location.range.start.line, 5);

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

    fn reference_source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = project_name
            }
        "#}
        .to_owned()
    }
}
