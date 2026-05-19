//! Language server runtime.
//!
//! This module owns the server lifecycle, runtime state, and LSP message
//! dispatch. Language feature implementations live in `crate::handlers`; LSP
//! capability and conversion helpers live in `crate::lsp`.

mod dispatch;
mod event_loop;
mod logging;
pub(crate) mod project;
mod state;
pub mod utils;

pub use event_loop::run;
pub use logging::init_logging;
pub use state::{Document, Documents, ServerState};
