use achitekfile::TextRange;

/// Prepare-rename target derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareRenameTarget {
    pub(super) range: TextRange,
    pub(super) placeholder: String,
}

impl PrepareRenameTarget {
    /// Returns the source range that should be renamed.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the placeholder name to show before rename.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
}
