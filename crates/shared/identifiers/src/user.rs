//! User identifier with `system` and `anonymous` constants.

crate::define_id!(UserId, schema);

impl UserId {
    /// Returns the canonical `"anonymous"` user identifier.
    pub fn anonymous() -> Self {
        Self("anonymous".to_string())
    }

    /// Returns the canonical `"system"` user identifier.
    pub fn system() -> Self {
        Self("system".to_string())
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
