crate::define_id!(UserId, schema);

impl UserId {
    pub fn anonymous() -> Self {
        Self("anonymous".to_string())
    }

    pub fn system() -> Self {
        Self("system".to_string())
    }

    pub fn is_system(&self) -> bool {
        self.0 == "system"
    }

    pub fn is_anonymous(&self) -> bool {
        self.0 == "anonymous"
    }
}
