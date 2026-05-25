//! User identifier.

crate::define_id!(UserId, schema);

impl UserId {
    pub fn anonymous() -> Self {
        Self("anonymous".to_owned())
    }

    pub fn system() -> Self {
        Self("system".to_owned())
    }

    pub fn bootstrap(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    pub fn is_system(&self) -> bool {
        self.0 == "system"
    }

    pub fn is_anonymous(&self) -> bool {
        self.0 == "anonymous"
    }
}
