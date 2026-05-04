use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::auth::UserType;

/// Claims extracted from a validated session JWT.
///
/// Returned by [`crate::auth::AuthValidationService::validate_request`]
/// when the token is well-formed; carries only the fields the rest of the
/// system needs to populate a
/// [`systemprompt_models::execution::context::RequestContext`].
#[derive(Debug, Clone)]
pub struct ValidatedSessionClaims {
    /// Subject of the JWT (`sub` claim) wrapped in a typed [`UserId`].
    pub user_id: UserId,
    /// `session_id` claim wrapped in a typed [`SessionId`].
    pub session_id: SessionId,
    /// Effective user type after admin-permission promotion.
    pub user_type: UserType,
}
