//! Request handler facade.
//!
//! Dispatch imports this module as a single request-handler namespace, while
//! individual feature handlers stay split into focused implementation modules.

pub use super::code_action::handle as handle_code_action;
pub use super::completion::handle as handle_completion;
pub use super::definition::handle as handle_definition;
pub use super::document_symbol::handle as handle_document_symbol;
pub use super::folding_range::handle as handle_folding_range;
pub use super::formatting::handle as handle_formatting;
pub use super::hover::handle as handle_hover;
pub use super::prepare_rename::handle as handle_prepare_rename;
pub use super::references::handle as handle_references;
pub use super::rename::handle as handle_rename;
pub use super::selection_range::handle as handle_selection_range;
pub use super::workspace_symbol::handle as handle_workspace_symbol;
