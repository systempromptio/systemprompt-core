//! Policy version identifier.

crate::define_id!(PolicyVersion);

impl PolicyVersion {
    /// Returns the canonical `"unversioned"` policy version.
    pub fn unversioned() -> Self {
        Self("unversioned".to_string())
    }
}
