use crate::workspace::{DocumentKind, Workspace, file_path_from_uri};
use lsp_types::Uri;
use std::collections::HashMap;

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

/// Runtime state shared by LSP request and notification handlers.
#[derive(Debug, Default)]
pub struct ServerState {
    /// Documents currently opened by the editor.
    pub documents: Documents,
    /// Known document kinds keyed by the string form of their URI.
    pub document_kinds: HashMap<String, DocumentKind>,
    /// Discovered blueprint projects in the active workspace.
    pub workspace: Workspace,
}

impl ServerState {
    /// Creates empty runtime state for a new server session.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates runtime state for a discovered workspace.
    pub fn with_workspace(workspace: Workspace) -> Self {
        Self {
            documents: Documents::new(),
            document_kinds: HashMap::new(),
            workspace,
        }
    }

    /// Records the kind of an opened document.
    pub fn set_document_kind(
        &mut self,
        uri: &Uri,
        language_id: Option<&str>,
        kind: Option<DocumentKind>,
    ) {
        let path = file_path_from_uri(uri);
        let kind = kind.unwrap_or_else(|| DocumentKind::classify(language_id, path.as_deref()));
        self.document_kinds.insert(uri.as_str().to_owned(), kind);
    }

    /// Removes cached metadata for a closed document.
    pub fn remove_document_kind(&mut self, uri: &Uri) {
        self.document_kinds.remove(uri.as_str());
    }

    /// Returns the best-known kind for a document URI.
    pub fn document_kind(&self, uri: &Uri) -> DocumentKind {
        self.document_kinds
            .get(uri.as_str())
            .copied()
            .unwrap_or_else(|| {
                let path = file_path_from_uri(uri);
                DocumentKind::classify(None, path.as_deref())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_document_kind_from_open_metadata() {
        let uri = "file:///workspace/rust/Cargo.toml.tera"
            .parse()
            .expect("URI should parse");
        let mut state = ServerState::new();

        state.set_document_kind(&uri, Some("tera"), None);

        assert_eq!(state.document_kind(&uri), DocumentKind::TeraTemplate);
    }

    #[test]
    fn falls_back_to_uri_classification_for_unknown_documents() {
        let uri = "file:///workspace/rust/achitekfile"
            .parse()
            .expect("URI should parse");
        let state = ServerState::new();

        assert_eq!(state.document_kind(&uri), DocumentKind::Achitekfile);
    }
}
