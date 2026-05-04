//! Execution-context identifier (one per logical conversation/task tree).

crate::define_id!(ContextId, generate, schema);

impl ContextId {
    /// Returns the canonical `"system"` context identifier.
    pub fn system() -> Self {
        Self("system".to_string())
    }

    /// Returns an empty (unset) context identifier.
    pub const fn empty() -> Self {
        Self(String::new())
    }

    /// Returns true if this is the empty (unset) identifier.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns true if this is the canonical `"system"` identifier.
    pub fn is_system(&self) -> bool {
        self.0 == "system"
    }

    /// Returns true if this is the canonical `"anonymous"` identifier.
    pub fn is_anonymous(&self) -> bool {
        self.0 == "anonymous"
    }
}
