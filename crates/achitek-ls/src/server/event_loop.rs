use crate::{
    arguments::CommunicationsChannel,
    lsp,
    server::{ServerState, dispatch},
    workspace::{self, Workspace},
};
use lsp_server::{Connection, Message};
use lsp_types::{InitializeParams, InitializeResult, ServerInfo};
use std::path::PathBuf;

pub fn run(channel: Option<CommunicationsChannel>) -> anyhow::Result<()> {
    let (connection, io_threads) = match channel.unwrap_or_default() {
        CommunicationsChannel::Stdio => {
            tracing::info!("using stdio communication channel");
            Connection::stdio()
        }
        chan => {
            tracing::error!("server does not support communication channel: {}", chan);
            std::process::exit(0);
        }
    };

    let hand_shake = serde_json::to_value(InitializeResult {
        capabilities: lsp::capabilities::make(),
        server_info: Some(ServerInfo {
            name: env!("CARGO_PKG_NAME").to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
    })?;

    tracing::info!("waiting for LSP initialize request");

    let (init_id, init_params) = match connection.initialize_start() {
        Ok(parts) => parts,
        Err(err) => {
            if err.channel_is_disconnected() {
                tracing::warn!(
                    "client disconnected during the beginning of initialization ceremony"
                );
                io_threads.join()?;
            }
            return Err(err.into());
        }
    };

    match connection.initialize_finish(init_id, hand_shake) {
        Ok(()) => {
            tracing::info!("LSP initialize handshake completed");
        }
        Err(err) => {
            if err.channel_is_disconnected() {
                tracing::warn!("client disconnected during the end of initialization ceremony");
                io_threads.join()?;
            }
            return Err(err.into());
        }
    }

    let workspace = workspace_root_from_init_params(init_params)
        .and_then(|root| match Workspace::discover(&root) {
            Ok(workspace) => Some(workspace),
            Err(error) => {
                tracing::warn!(
                    root = %root.display(),
                    error = %error,
                    "failed to discover blueprint workspace"
                );
                None
            }
        })
        .unwrap_or_default();

    let mut state = ServerState::with_workspace(workspace);

    for msg in &connection.receiver {
        match msg {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    tracing::info!("received LSP shutdown request");
                    break;
                }
                dispatch::handle_request(&connection, &state, request)?;
            }
            Message::Notification(notification) => {
                dispatch::handle_notification(&connection, &mut state, notification)?;
            }
            Message::Response(response) => {
                tracing::debug!(response = ?response, "received unexpected LSP response");
            }
        }
    }

    tracing::info!("joining LSP IO threads");
    io_threads.join()?;
    tracing::info!("LSP server run loop exited");

    Ok(())
}

fn workspace_root_from_init_params(init_params: serde_json::Value) -> Option<PathBuf> {
    let params = serde_json::from_value::<InitializeParams>(init_params)
        .inspect_err(|error| tracing::warn!(%error, "failed to parse initialize params"))
        .ok()?;

    let workspace_folder_root = params.workspace_folders.as_ref().and_then(|folders| {
        folders
            .iter()
            .find_map(|folder| workspace::file_path_from_uri(&folder.uri))
    });

    workspace_folder_root.or_else(|| root_uri_workspace_path(params))
}

#[allow(deprecated)]
fn root_uri_workspace_path(params: InitializeParams) -> Option<PathBuf> {
    params
        .root_uri
        .and_then(|uri| workspace::file_path_from_uri(&uri))
}
