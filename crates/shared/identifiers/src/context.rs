crate::define_id!(ContextId, generate, schema);

impl ContextId {
    pub fn system() -> Self {
        Self("system".to_string())
    }

    pub const fn empty() -> Self {
        Self(String::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_system(&self) -> bool {
        self.0 == "system"
    }

    pub fn is_anonymous(&self) -> bool {
        self.0 == "anonymous"
    }
}
