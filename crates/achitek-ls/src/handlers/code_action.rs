//! Handler for the LSP `textDocument/codeAction` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_codeAction>
//!
//! Clients send this request when the user asks for fixes near a diagnostic.
//! The server returns workspace edits for project-level problems that can be
//! repaired safely.

use crate::{
    lsp::project_diagnostics,
    server::{ServerState, project::ProjectContext, utils},
    workspace::DocumentKind,
};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Diagnostic, NumberOrString, Position, Range, TextEdit, Uri, WorkspaceEdit,
};
use std::collections::HashMap;

/// Handles a `textDocument/codeAction` request.
pub fn handle(
    state: &ServerState,
    params: CodeActionParams,
) -> anyhow::Result<Option<CodeActionResponse>> {
    if !allows_quickfix(&params) {
        return Ok(Some(Vec::new()));
    }

    match state.document_kind(&params.text_document.uri) {
        DocumentKind::TeraTemplate => tera_code_actions(state, params),
        DocumentKind::Achitekfile | DocumentKind::Manifest | DocumentKind::Unknown => {
            Ok(Some(Vec::new()))
        }
    }
}

fn tera_code_actions(
    state: &ServerState,
    params: CodeActionParams,
) -> anyhow::Result<Option<CodeActionResponse>> {
    let uri = params.text_document.uri;
    let Some(diagnostic) = unknown_prompt_diagnostic(&params.context.diagnostics) else {
        return Ok(Some(Vec::new()));
    };
    if !ranges_intersect(params.range, diagnostic.range) {
        return Ok(Some(Vec::new()));
    }

    let Some(template_path) = utils::file_path_from_uri(&uri) else {
        tracing::debug!(?uri, "template code action skipped for non-file URI");
        return Ok(Some(Vec::new()));
    };
    let Some(project) = ProjectContext::for_template_path(state, &template_path) else {
        tracing::debug!(
            ?uri,
            "template code action skipped because no project was found"
        );
        return Ok(Some(Vec::new()));
    };
    let source = project.template_source(&uri, &template_path)?;
    let Some(prompt_name) = utils::reference_at_position(&source, diagnostic.range.start) else {
        tracing::debug!(
            ?uri,
            range = ?diagnostic.range,
            "template code action skipped because diagnostic range did not resolve to a prompt reference"
        );
        return Ok(Some(Vec::new()));
    };

    let achitek_uri = project.achitekfile_uri()?;
    let achitek_source = project.achitekfile_source()?;
    let action = create_string_prompt_action(
        achitek_uri,
        &achitek_source,
        &prompt_name,
        diagnostic.clone(),
    );

    Ok(Some(vec![CodeActionOrCommand::CodeAction(action)]))
}

#[allow(clippy::mutable_key_type)]
fn create_string_prompt_action(
    uri: Uri,
    source: &str,
    prompt_name: &str,
    diagnostic: Diagnostic,
) -> CodeAction {
    let insert_position = eof_position(source);
    let mut changes = HashMap::new();
    changes.insert(
        uri,
        vec![TextEdit {
            range: Range {
                start: insert_position,
                end: insert_position,
            },
            new_text: prompt_block_insert_text(source, prompt_name),
        }],
    );

    CodeAction {
        title: format!("Create string prompt `{prompt_name}`"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        is_preferred: Some(true),
        ..CodeAction::default()
    }
}

fn prompt_block_insert_text(source: &str, prompt_name: &str) -> String {
    let prompt = format!("prompt \"{prompt_name}\" {{\n  type = string\n}}\n");

    if source.trim().is_empty() {
        prompt
    } else if source.ends_with('\n') {
        format!("\n{prompt}")
    } else {
        format!("\n\n{prompt}")
    }
}

fn eof_position(source: &str) -> Position {
    let mut line = 0;
    let mut character = 0;

    for ch in source.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += u32::try_from(ch.len_utf8()).expect("character width should fit into u32");
        }
    }

    Position { line, character }
}

fn unknown_prompt_diagnostic(diagnostics: &[Diagnostic]) -> Option<&Diagnostic> {
    diagnostics.iter().find(|diagnostic| {
        diagnostic.code.as_ref().is_some_and(|code| match code {
            NumberOrString::String(code) => code == project_diagnostics::UNKNOWN_PROMPT_CODE,
            NumberOrString::Number(_) => false,
        })
    })
}

fn allows_quickfix(params: &CodeActionParams) -> bool {
    params.context.only.as_ref().is_none_or(|kinds| {
        kinds
            .iter()
            .any(|kind| kind.as_str() == CodeActionKind::QUICKFIX.as_str())
    })
}

fn ranges_intersect(left: Range, right: Range) -> bool {
    left.start <= right.end && right.start <= left.end
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::{Documents, utils};
    use indoc::indoc;
    use lsp_server::{Connection, Message, Request, RequestId, Response};
    use lsp_types::{
        CodeActionContext, TextDocumentIdentifier, request::CodeActionRequest,
        request::Request as _,
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
    #[allow(clippy::mutable_key_type)]
    fn handle_template_code_action_creates_string_prompt() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-code-action")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(&template_path, r#"name = "{{ project_name }}""#)?;
        let achitek_uri = utils::path_to_uri(&achitek_path)?;
        let template_uri = utils::path_to_uri(&template_path)?;
        let diagnostic_range = Range {
            start: Position {
                line: 0,
                character: 11,
            },
            end: Position {
                line: 0,
                character: 23,
            },
        };
        let diagnostic = Diagnostic {
            range: diagnostic_range,
            code: Some(NumberOrString::String(
                project_diagnostics::UNKNOWN_PROMPT_CODE.to_owned(),
            )),
            message: "unknown prompt reference `project_name`".to_owned(),
            ..Diagnostic::default()
        };
        let request = Request::new(
            RequestId::from(1_i32),
            CodeActionRequest::METHOD.to_owned(),
            CodeActionParams {
                text_document: TextDocumentIdentifier { uri: template_uri },
                range: diagnostic_range,
                context: CodeActionContext {
                    diagnostics: vec![diagnostic],
                    only: Some(vec![CodeActionKind::QUICKFIX]),
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            },
        );
        let (server_connection, client_connection) = Connection::memory();
        let documents = Documents::new();

        handle(&server_connection, &request, &documents)?;

        let response = recv_response(&client_connection)?;
        let actions: Option<CodeActionResponse> =
            serde_json::from_value(response.result.expect("response should contain a result"))?;
        let actions = actions.expect("code action response should be present");
        assert_eq!(actions.len(), 1);
        let CodeActionOrCommand::CodeAction(action) = &actions[0] else {
            panic!("expected code action");
        };
        assert_eq!(action.title, "Create string prompt `project_name`");
        let edit = action.edit.as_ref().expect("action should include an edit");
        let changes = edit.changes.as_ref().expect("edit should include changes");
        let edits = changes
            .get(&achitek_uri)
            .expect("Achitekfile should receive the edit");
        assert_eq!(
            edits[0].new_text,
            "\nprompt \"project_name\" {\n  type = string\n}\n"
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

    fn source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }
        "#}
        .to_owned()
    }
}
