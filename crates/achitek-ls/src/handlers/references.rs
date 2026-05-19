//! Handler for the LSP `textDocument/references` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_references>
//!
//! Clients send this request when the user asks to find all uses of the symbol
//! at a cursor position. For Achitekfiles, this returns prompt declaration and
//! dependency-expression reference locations, plus prompt references in nearby
//! `.tera` templates.

#[cfg(test)]
use crate::server::{Document, Documents};
use crate::{
    editor,
    server::{ServerState, project::ProjectContext, utils},
    syntax,
    workspace::DocumentKind,
};
use anyhow::Context;
use lsp_types::{Location, Position, Range, ReferenceParams, Uri};

/// Handles a `textDocument/references` request.
pub fn handle(
    state: &ServerState,
    params: ReferenceParams,
) -> anyhow::Result<Option<Vec<Location>>> {
    let text_document_position = params.text_document_position;
    let uri = text_document_position.text_document.uri;
    let position = text_document_position.position;

    match state.document_kind(&uri) {
        DocumentKind::Achitekfile => {
            achitekfile_references(state, uri, position, params.context.include_declaration)
        }
        DocumentKind::TeraTemplate => {
            tera_references(state, uri, position, params.context.include_declaration)
        }
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(None),
    }
}

fn achitekfile_references(
    state: &ServerState,
    uri: Uri,
    position: Position,
    include_declaration: bool,
) -> anyhow::Result<Option<Vec<Location>>> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(None);
    };

    let analysis = editor::build(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let cursor_position = to_text_position(position);
    let prompt_name = analysis.prompt_name(cursor_position).map(str::to_owned);
    let mut locations = analysis
        .references(cursor_position, include_declaration)
        .into_iter()
        .map(|target| Location::new(uri.clone(), to_lsp_range(target.range())))
        .collect::<Vec<_>>();

    if let (Some(prompt_name), Some(project)) = (prompt_name, ProjectContext::for_uri(state, &uri))
    {
        locations.extend(project.scan_template_references(&prompt_name)?);
    }

    Ok(Some(locations))
}

fn tera_references(
    state: &ServerState,
    uri: Uri,
    position: Position,
    include_declaration: bool,
) -> anyhow::Result<Option<Vec<Location>>> {
    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template references skipped for non-file URI");
        return Ok(None);
    };

    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template references skipped because no project was found"
        );
        return Ok(None);
    };
    let source = project.template_source(&uri, &template_path)?;
    let Some(prompt_name) = utils::reference_at_position(&source, position) else {
        tracing::debug!(?uri, ?position, "no template reference under cursor");
        return Ok(None);
    };

    let achitek_uri = project.achitekfile_uri()?;
    let achitek_source = project.achitekfile_source()?;
    let analysis = editor::build(&achitek_source).with_context(|| {
        format!(
            "failed to analyze `{}`",
            project.achitekfile_path().display()
        )
    })?;
    let Some(symbol) = analysis
        .symbols()
        .iter()
        .find(|symbol| symbol.kind() == editor::SymbolKind::Prompt && symbol.name() == prompt_name)
    else {
        tracing::debug!(
            ?uri,
            reference = prompt_name,
            target = ?achitek_uri,
            "template references skipped because prompt was not found"
        );
        return Ok(None);
    };

    let cursor_position = symbol.selection_range().start_position;
    let mut locations = analysis
        .references(cursor_position, include_declaration)
        .into_iter()
        .map(|target| Location::new(achitek_uri.clone(), to_lsp_range(target.range())))
        .collect::<Vec<_>>();

    locations.extend(project.scan_template_references(&prompt_name)?);

    Ok(Some(locations))
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
    use lsp_server::{Connection, Message, Request, RequestId, Response};
    use lsp_types::{
        ReferenceContext, TextDocumentIdentifier, TextDocumentPositionParams,
        request::{References, Request as LspRequest},
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
    fn handle_references_request() -> anyhow::Result<()> {
        let (server_connection, client_connection) = Connection::memory();
        let uri = test_uri()?;
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id.clone(),
            References::METHOD.to_owned(),
            ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 13,
                        character: 16,
                    },
                },
                context: ReferenceContext {
                    include_declaration: true,
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
        let locations: Option<Vec<Location>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let locations = locations.expect("references should be available");
        assert_eq!(locations.len(), 2);
        assert!(
            locations
                .iter()
                .any(|location| location.range.start.line == 5)
        );
        assert!(
            locations
                .iter()
                .any(|location| location.range.start.line == 13)
        );

        Ok(())
    }

    #[test]
    fn handle_references_request_includes_template_references() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-references")?;
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
        let uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let (server_connection, client_connection) = Connection::memory();
        let request_id = RequestId::from(1_i32);
        let request = Request::new(
            request_id,
            References::METHOD.to_owned(),
            ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position {
                        line: 5,
                        character: 10,
                    },
                },
                context: ReferenceContext {
                    include_declaration: true,
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
        let locations: Option<Vec<Location>> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let locations = locations.expect("references should be available");
        assert_eq!(locations.len(), 3);
        assert!(
            locations
                .iter()
                .any(|location| location.uri == template_uri && location.range.start.line == 1)
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn handle_template_references_request_uses_workspace_project() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-workspace-template-references")?;
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
        fs::write(
            &template_path,
            indoc! {r#"[package]
                name = "{{project_name}}"
            "#},
        )?;
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

        let locations = super::handle(
            &state,
            ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: template_uri.clone(),
                    },
                    position: Position {
                        line: 1,
                        character: 13,
                    },
                },
                context: ReferenceContext {
                    include_declaration: true,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        )?
        .expect("references should be available");

        assert_eq!(locations.len(), 3);
        assert!(
            locations
                .iter()
                .any(|location| location.uri == achitek_uri && location.range.start.line == 5)
        );
        assert!(
            locations
                .iter()
                .any(|location| location.uri == template_uri && location.range.start.line == 1)
        );

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
