use crate::protocol::Documents;
use lsp_server::Connection;
use lsp_types::Uri;

/// Publishes LSP diagnostics for a blueprint project.
///
/// Uses the editor-owned text from the open document store, not the file system,
/// then maps analysis diagnostics into `textDocument/publishDiagnostics`.
/// Unknown URIs are ignored because diagnostics can only be computed for
/// documents currently tracked by the runtime.
///
/// [PublishDiagnostic Notification Spec]
///
/// [PublishDiagnostic Notification Spec]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_publishDiagnostics
pub fn publish(connection: &Connection, uri: &Uri, documents: &Documents) -> anyhow::Result<()> {
    tracing::debug!(?uri, "publishing diagnostics");

    Ok(())
}

pub fn clear(connection: &Connection, uri: &Uri) -> anyhow::Result<()> {
    tracing::debug!(?uri, "clearing diagnostics");

    Ok(())
}
