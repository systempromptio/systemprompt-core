use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoginOutput {
    pub user_id: String,
    pub email: String,
    pub session_id: String,
    pub token: String,
    pub expires_in_hours: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiOutput {
    pub user_id: String,
    pub email: String,
    pub session_id: Option<String>,
    pub is_admin: bool,
}
