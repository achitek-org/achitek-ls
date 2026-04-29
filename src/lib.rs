//! Implementation crate for the `achitek-ls` language server.
//!
//! The public Rust API is intentionally not the primary interface for this
//! project. Users normally run the `achitek-ls` binary from an editor or LSP
//! client. Most modules are exported only so the binary target can compose the
//! implementation, and are hidden from generated user-facing documentation.

#[doc(hidden)]
pub mod analysis;
pub mod arguments;
#[doc(hidden)]
pub mod capabilities;
#[doc(hidden)]
mod server;
#[doc(hidden)]
pub mod syntax;

#[doc(hidden)]
pub use server::Server;
