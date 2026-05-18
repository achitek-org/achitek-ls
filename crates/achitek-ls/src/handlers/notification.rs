//! Notification handler facade.
//!
//! Dispatch imports this module as a single notification-handler namespace,
//! while individual notification handlers stay split by LSP method.

pub use super::did_change::handle as handle_did_change;
pub use super::did_close::handle as handle_did_close;
pub use super::did_open::handle as handle_did_open;
