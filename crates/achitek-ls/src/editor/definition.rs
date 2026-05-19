use achitekfile::TextRange;

/// Definition target derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionTarget {
    pub(super) range: TextRange,
    pub(super) selection_range: TextRange,
}

impl DefinitionTarget {
    /// Returns the full range of the definition target.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the preferred selection range for the definition target.
    pub fn selection_range(&self) -> TextRange {
        self.selection_range
    }
}
