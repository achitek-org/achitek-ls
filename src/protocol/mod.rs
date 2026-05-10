//! Language Server Protocol
//!
//! This module is responsible for protocol request, notification & response handling,
//! diagnostic publishing, and conversion between types and lsp types.

mod diagnostics;
mod document;
mod handlers;

pub use document::{Document, Documents};
pub use handlers::{
    did_change::handle as handle_did_change, did_close::handle as handle_did_close,
    did_open::handle as handle_did_open,
};
