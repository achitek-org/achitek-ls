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

        let init_value = serde_json::to_value(InitializeResult {
            capabilities: capabilities::make(),
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })?;

        tracing::info!("waiting for LSP initialize request");
        let _init_params = match connection.initialize(init_value) {
            Ok(params) => {
                tracing::info!("LSP initialize handshake completed");
                params
            }
            Err(err) => {
                if err.channel_is_disconnected() {
                    tracing::warn!("client disconnected during initialize");
                    io_threads.join()?;
                }
                return Err(err.into());
            }
        };

        for msg in &connection.receiver {
            match msg {
                Message::Request(req) => {
                    tracing::debug!(method = %req.method, id = ?req.id, "received LSP request");
                    if connection.handle_shutdown(&req)? {
                        tracing::info!("received LSP shutdown request");
                        break;
                    }

                    match req.method.as_str() {
                        "textDocument/documentSymbol" => {
                            handlers::document_symbol::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/formatting" => {
                            handlers::formatting::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/foldingRange" => {
                            handlers::folding_range::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/selectionRange" => {
                            handlers::selection_range::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/hover" => {
                            handlers::hover::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/completion" => {
                            handlers::completion::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/definition" => {
                            handlers::definition::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/references" => {
                            handlers::references::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/rename" => {
                            handlers::rename::handle(&connection, &req, &documents)?;
                        }
                        "textDocument/prepareRename" => {
                            handlers::prepare_rename::handle(&connection, &req, &documents)?;
                        }
                        "workspace/symbol" => {
                            handlers::workspace_symbol::handle(&connection, &req, &documents)?;
                        }
                        _ => {
                            tracing::warn!(method = %req.method, "unable to handle LSP request");

                            let response = Response {
                                id: req.id.clone(),
                                result: None,
                                error: None,
                            };

                            connection.sender.send(Message::Response(response))?;
                        }
                    }
                }
                Message::Notification(note) => match note.method.as_str() {
                    "textDocument/didOpen" => {
                        tracing::debug!(method = %note.method, "received LSP notification");
                        handlers::did_open::handle(&connection, &note, &mut documents)?;
                    }
                    "textDocument/didChange" => {
                        tracing::debug!(method = %note.method, "received LSP notification");
                        handlers::did_change::handle(&connection, &note, &mut documents)?;
                    }
                    "textDocument/didClose" => {
                        tracing::debug!(method = %note.method, "received LSP notification");
                        handlers::did_close::handle(&connection, &note, &mut documents)?;
                    }
                    _ => {
                        tracing::debug!(method = %note.method, "ignoring LSP notification");
                    }
                },
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
