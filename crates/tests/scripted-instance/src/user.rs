//! The scripted user and its personal access token.

use systemprompt_identifiers::{TenantId, UserId};
use systemprompt_models::auth::API_KEY_PREFIX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestUser {
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    pub email: String,
    pub display_name: String,
    pub roles: Vec<String>,
}

impl Default for TestUser {
    fn default() -> Self {
        Self {
            user_id: UserId::new("scripted-user"),
            tenant_id: None,
            email: "scripted-user@example.test".to_owned(),
            display_name: "Scripted User".to_owned(),
            roles: vec!["user".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPat(String);

impl TestPat {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn fixture() -> Self {
        Self::new(format!("{API_KEY_PREFIX}scripted.instance-pat"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
