use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::auth::UserType;

#[derive(Debug, Clone)]
pub struct ValidatedSessionClaims {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub user_type: UserType,
}
