use achitekfile::TextRange;

/// Reference target derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTarget {
    pub(super) range: TextRange,
}

impl ReferenceTarget {
    /// Returns the source range for the reference target.
    pub fn range(&self) -> TextRange {
        self.range
    }
}
