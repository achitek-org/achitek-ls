/// In-memory state for an open text document.
#[derive(Debug, Clone)]
pub struct Document {
    /// Latest version number reported by the client.
    pub version: i32,
    /// Latest full document text reported by the client.
    pub text: String,
}

impl Document {
    pub fn get_version(&self) -> i32 {
        self.version
    }

    pub fn set_version(&mut self, new_version: i32) {
        self.version = new_version;
    }

    pub fn get_text(&self) -> String {
        self.text.clone()
    }

    pub fn set_text(&mut self, new_text: String) {
        self.text = new_text;
    }
}

/// Open documents keyed by the string form of their URI.
///
/// `lsp_types::Uri` carries interior cache state, so keeping URI strings as
/// keys avoids Clippy's `mutable_key_type` warning while preserving the exact
/// client URI for lookup.
pub type Documents = rustc_hash::FxHashMap<String, Document>;
