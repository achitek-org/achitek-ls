use crate::{arguments::CommunicationsChannel, protocol};
use lsp_server::{Connection, Message};
use lsp_types::{
    InitializeResult, ServerInfo,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification,
    },
};
use rustc_hash::FxHashMap;

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
        capabilities: super::capabilities::make(),
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

    let _init_params = init_params;

    let mut documents = FxHashMap::default();

    for msg in &connection.receiver {
        match msg {
            Message::Request(request) => {
                //
            }
            Message::Notification(notification) => match notification.method.as_str() {
                DidOpenTextDocument::METHOD => {
                    protocol::handle_did_open(&connection, &notification, &mut documents)?;
                }
                DidCloseTextDocument::METHOD => {
                    protocol::handle_did_close(&connection, &notification, &mut documents)?;
                }
                DidChangeTextDocument::METHOD => {
                    protocol::handle_did_change(&connection, &notification, &mut documents)?;
                }
                _ => {
                    tracing::debug!(method = %notification.method, "ignoring LSP notification");
                }
            },
            Message::Response(response) => {
                //
            }
        }
    }

    tracing::info!("joining LSP IO threads");
    io_threads.join()?;
    tracing::info!("LSP server run loop exited");

    Ok(())
}
