//! LSP server runtime and message dispatch.
//!
//! This module owns the server lifecycle: choosing a communication channel,
//! performing the LSP initialize handshake, maintaining the in-memory document
//! store, and dispatching incoming requests and notifications to handler
//! modules.
//!
//! The server stores open documents in memory so language features operate on
//! the editor buffer rather than on potentially stale files on disk.

use crate::{arguments::CommunicationsChannel, capabilities};
use lsp_server::{Connection, IoThreads, Message, Response};
use lsp_types::{InitializeResult, ServerInfo};
use std::collections::HashMap;

mod handlers;
mod utils;

/// In-memory state for an open text document.
#[derive(Debug, Clone)]
pub struct Document {
    /// Latest version number reported by the client.
    pub version: i32,
    /// Latest full document text reported by the client.
    pub text: String,
}

/// Open documents keyed by the string form of their URI.
///
/// `lsp_types::Uri` carries interior cache state, so keeping URI strings as
/// keys avoids Clippy's `mutable_key_type` warning while preserving the exact
/// client URI for lookup.
pub type Documents = HashMap<String, Document>;

/// Language server runtime.
///
/// A `Server` owns the transport connection, the background IO threads for
/// that transport, and the open-document state used by request handlers.
pub struct Server {
    /// Message connection used to receive client messages and send responses.
    pub connection: Connection,
    /// Background transport threads that must be joined during shutdown.
    pub io_threads: IoThreads,
    /// Open documents keyed by URI.
    pub documents: Documents,
}

impl Server {
    /// Creates a server for the requested communication channel.
    ///
    /// Currently only stdio is supported. Missing channels default to stdio.
    /// Unsupported channels log an error and exit the process.
    pub fn new(channel: Option<CommunicationsChannel>) -> Self {
        let (connection, io_threads) = Server::resolve_communications_channel(channel);
        Self {
            connection,
            io_threads,
            documents: HashMap::new(),
        }
    }

    /// Runs the LSP initialize handshake and main message loop.
    ///
    /// The loop dispatches requests and notifications until the client sends a
    /// shutdown request or the connection closes. After the loop exits, the
    /// underlying IO threads are joined before returning.
    pub fn run(self) -> anyhow::Result<()> {
        let Self {
            connection,
            io_threads,
            mut documents,
        } = self;

        let init_value = initialize_result_value()?;

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
        match connection.initialize_finish(init_id, init_value) {
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

        for msg in &connection.receiver {
            match msg {
                Message::Request(request) => {
                    tracing::debug!(method = %request.method, id = ?request.id, "received LSP request");
                    if connection.handle_shutdown(&request)? {
                        tracing::info!("received LSP shutdown request");
                        break;
                    }

                    match request.method.as_str() {
                        "textDocument/documentSymbol" => {
                            handlers::document_symbol::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/formatting" => {
                            handlers::formatting::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/foldingRange" => {
                            handlers::folding_range::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/selectionRange" => {
                            handlers::selection_range::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/hover" => {
                            handlers::hover::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/completion" => {
                            handlers::completion::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/definition" => {
                            handlers::definition::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/references" => {
                            handlers::references::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/rename" => {
                            handlers::rename::handle(&connection, &request, &documents)?;
                        }
                        "textDocument/prepareRename" => {
                            handlers::prepare_rename::handle(&connection, &request, &documents)?;
                        }
                        "workspace/symbol" => {
                            handlers::workspace_symbol::handle(&connection, &request, &documents)?;
                        }
                        _ => {
                            tracing::warn!(method = %request.method, "unable to handle LSP request");

                            let response = Response {
                                id: request.id.clone(),
                                result: None,
                                error: None,
                            };

                            connection.sender.send(Message::Response(response))?;
                        }
                    }

                    tracing::debug!(method = %request.method, "successfully handled received LSP request");
                }
                Message::Notification(notification) => {
                    match notification.method.as_str() {
                        "textDocument/didOpen" => {
                            handlers::did_open::handle(&connection, &notification, &mut documents)?;
                        }
                        "textDocument/didChange" => {
                            handlers::did_change::handle(
                                &connection,
                                &notification,
                                &mut documents,
                            )?;
                        }
                        "textDocument/didClose" => {
                            handlers::did_close::handle(
                                &connection,
                                &notification,
                                &mut documents,
                            )?;
                        }
                        _ => {
                            tracing::debug!(method = %notification.method, "ignoring LSP notification");
                        }
                    }

                    tracing::debug!(method = %notification.method, "successfully handled received LSP notification");
                }
                Message::Response(resp) => {
                    tracing::debug!(response = ?resp, "received unexpected LSP response");
                }
            }
        }

        tracing::info!("joining LSP IO threads");
        io_threads.join()?;

        tracing::info!("LSP server run loop exited");
        Ok(())
    }

    /// Creates the underlying LSP connection for a communication channel.
    fn resolve_communications_channel(
        channel: Option<CommunicationsChannel>,
    ) -> (Connection, IoThreads) {
        match channel.unwrap_or_default() {
            CommunicationsChannel::Stdio => {
                tracing::info!("using stdio communication channel");
                Connection::stdio()
            }
            chan => {
                tracing::error!("server does not support communication channel: {}", chan);
                std::process::exit(0);
            }
        }
    }
}

fn initialize_result_value() -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::to_value(InitializeResult {
        capabilities: capabilities::make(),
        server_info: Some(ServerInfo {
            name: env!("CARGO_PKG_NAME").to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
    })?)
}

#[cfg(test)]
mod tests {
    use super::initialize_result_value;

    #[test]
    fn initialize_result_advertises_capabilities_at_lsp_top_level() -> anyhow::Result<()> {
        let value = initialize_result_value()?;

        assert_eq!(value["capabilities"]["hoverProvider"], true);
        assert_eq!(value["capabilities"]["definitionProvider"], true);
        assert_eq!(value["capabilities"]["referencesProvider"], true);
        assert!(value["capabilities"]["renameProvider"]["prepareProvider"].as_bool() == Some(true));
        assert!(value["capabilities"]["capabilities"].is_null());

        Ok(())
    }
}
