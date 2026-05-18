//! Typed LSP message dispatch.
//!
//! The handlers still receive raw `lsp_server` messages, but this dispatch
//! layer matches on typed `lsp_types` method constants instead of string
//! literals. That keeps the event loop small and gives us one place to tighten
//! handler signatures later.

use crate::server::{Documents, ServerState};
use anyhow::Context;
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    },
    request::{
        Completion, DocumentSymbolRequest, FoldingRangeRequest, Formatting, GotoDefinition,
        HoverRequest, PrepareRenameRequest, References, Rename, Request as _,
        SelectionRangeRequest, WorkspaceSymbolRequest,
    },
};
use serde::{Serialize, de::DeserializeOwned};

pub fn handle_request(
    connection: &Connection,
    state: &ServerState,
    request: Request,
) -> anyhow::Result<()> {
    tracing::debug!(method = %request.method, id = ?request.id, "received LSP request");

    use crate::handlers::request as handlers;

    match request.method.as_str() {
        DocumentSymbolRequest::METHOD => on_request::<DocumentSymbolRequest>(
            connection,
            &state.documents,
            request,
            handlers::handle_document_symbol,
        ),
        Formatting::METHOD => on_request::<Formatting>(
            connection,
            &state.documents,
            request,
            handlers::handle_formatting,
        ),
        FoldingRangeRequest::METHOD => on_request::<FoldingRangeRequest>(
            connection,
            &state.documents,
            request,
            handlers::handle_folding_range,
        ),
        SelectionRangeRequest::METHOD => on_request::<SelectionRangeRequest>(
            connection,
            &state.documents,
            request,
            handlers::handle_selection_range,
        ),
        HoverRequest::METHOD => on_request::<HoverRequest>(
            connection,
            &state.documents,
            request,
            handlers::handle_hover,
        ),
        Completion::METHOD => on_request::<Completion>(
            connection,
            &state.documents,
            request,
            handlers::handle_completion,
        ),
        GotoDefinition::METHOD => on_request::<GotoDefinition>(
            connection,
            &state.documents,
            request,
            handlers::handle_definition,
        ),
        References::METHOD => on_request::<References>(
            connection,
            &state.documents,
            request,
            handlers::handle_references,
        ),
        Rename::METHOD => on_request::<Rename>(
            connection,
            &state.documents,
            request,
            handlers::handle_rename,
        ),
        PrepareRenameRequest::METHOD => on_request::<PrepareRenameRequest>(
            connection,
            &state.documents,
            request,
            handlers::handle_prepare_rename,
        ),
        WorkspaceSymbolRequest::METHOD => on_request::<WorkspaceSymbolRequest>(
            connection,
            &state.documents,
            request,
            handlers::handle_workspace_symbol,
        ),
        _ => respond_method_not_found(connection, request),
    }
}

pub fn handle_notification(
    connection: &Connection,
    state: &mut ServerState,
    notification: Notification,
) -> anyhow::Result<()> {
    tracing::debug!(method = %notification.method, "received LSP notification");

    use crate::handlers::notification as handlers;

    match notification.method.as_str() {
        DidOpenTextDocument::METHOD => on_notification::<DidOpenTextDocument>(
            connection,
            state,
            notification,
            handlers::handle_did_open,
        ),
        DidCloseTextDocument::METHOD => on_notification::<DidCloseTextDocument>(
            connection,
            state,
            notification,
            handlers::handle_did_close,
        ),
        DidChangeTextDocument::METHOD => on_notification::<DidChangeTextDocument>(
            connection,
            state,
            notification,
            handlers::handle_did_change,
        ),
        _ => {
            tracing::debug!(method = %notification.method, "ignoring LSP notification");
            Ok(())
        }
    }
}

fn on_request<R>(
    connection: &Connection,
    documents: &Documents,
    request: Request,
    handler: fn(&Documents, R::Params) -> anyhow::Result<R::Result>,
) -> anyhow::Result<()>
where
    R: lsp_types::request::Request,
    R::Params: DeserializeOwned,
    R::Result: Serialize,
{
    let response = match serde_json::from_value(request.params) {
        Ok(params) => match handler(documents, params) {
            Ok(result) => Response::new_ok(request.id, result),
            Err(error) => Response::new_err(
                request.id,
                lsp_server::ErrorCode::InternalError as i32,
                error.to_string(),
            ),
        },
        Err(error) => Response::new_err(
            request.id,
            lsp_server::ErrorCode::InvalidParams as i32,
            error.to_string(),
        ),
    };

    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send request response")
}

fn on_notification<N>(
    connection: &Connection,
    state: &mut ServerState,
    notification: Notification,
    handler: fn(&Connection, &mut ServerState, N::Params) -> anyhow::Result<()>,
) -> anyhow::Result<()>
where
    N: lsp_types::notification::Notification,
    N::Params: DeserializeOwned,
{
    let params = serde_json::from_value(notification.params)
        .with_context(|| format!("failed to parse {} params", N::METHOD))?;
    handler(connection, state, params)
}

fn respond_method_not_found(connection: &Connection, request: Request) -> anyhow::Result<()> {
    tracing::warn!(method = %request.method, "unable to handle LSP request");
    let response = Response::new_err(
        request.id,
        lsp_server::ErrorCode::MethodNotFound as i32,
        "unknown request".to_owned(),
    );
    connection
        .sender
        .send(Message::Response(response))
        .context("failed to send unknown-request response")
}
