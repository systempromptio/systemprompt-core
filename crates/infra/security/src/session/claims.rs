use systemprompt_models::auth::UserType;

#[derive(Debug, Clone)]
pub struct ValidatedSessionClaims {
    pub user_id: String,
    pub session_id: String,
    pub user_type: UserType,
}
