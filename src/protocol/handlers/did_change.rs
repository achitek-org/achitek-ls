use crate::protocol::{Documents, diagnostics};
use anyhow::Context;
use lsp_server::{Connection, Notification};
use lsp_types::{DidChangeTextDocumentParams, TextDocumentContentChangeEvent};

/// Handles a 'textDocument/didChange' notification. See [Spec]
///
/// A client sends this to a server to signal changes to a document.
/// At the moment, this server advertises the TextDocumentSyncKind::FULL
/// capability, therefore we apply full-document content changes.
///
/// [Spec]: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didChange
pub fn handle(
    connection: &Connection,
    notification: &Notification,
    documents: &mut Documents,
) -> anyhow::Result<()> {
    let params: DidChangeTextDocumentParams = serde_json::from_value(notification.params.clone())
        .context("failed to parse didChange notification")?;
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    let change_count = params.content_changes.len();

    if let Some(document) = documents.get_mut(uri.as_str()) {
        document.set_version(version);
        document.set_text(apply_changes(&document.text, &params.content_changes));
        tracing::debug!(?uri, version, change_count, "changed document");
        diagnostics::publish(connection, &uri, documents)?;
    } else {
        tracing::warn!(
            ?uri,
            version,
            change_count,
            "received change for unknown document"
        );
    }

    Ok(())
}

/// Applies full document content changes.
fn apply_changes(current: &str, changes: &[TextDocumentContentChangeEvent]) -> String {
    for change in changes {
        if change.range.is_some() {
            tracing::warn!(
                "received incremental change, but this server advertises full document sync."
            );
        }
    }
    // PERF: If this becomes a bottleneck. Refer to https://github.com/achitek-org/achitek-ls/issues/25
    changes
        .last()
        .map(|change| change.text.clone())
        .unwrap_or_else(|| current.to_owned())
}
