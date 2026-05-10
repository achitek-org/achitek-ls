//! Languate Server
//!
//! This module is responsible for defining the advertised capabilities,
//! provide log inititalization, select the communication channel as defined
//! by caller, perform the initialization handshake and start the LSP runtime.

mod capabilities;
mod logging;
mod server;

pub use logging::init_logging;
pub use server::run;
