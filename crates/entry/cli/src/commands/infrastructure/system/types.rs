use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{SessionId, SessionToken, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginOutput {
    pub user_id: UserId,
    pub email: String,
    pub session_id: SessionId,
    pub token: SessionToken,
    pub expires_in_hours: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiOutput {
    pub user_id: UserId,
    pub email: String,
    pub session_id: Option<SessionId>,
    pub is_admin: bool,
}
