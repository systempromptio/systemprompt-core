//! Validated session-claim types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AccessTokenId, SessionId, UserId};
use systemprompt_models::auth::UserType;

#[derive(Debug, Clone)]
pub struct ValidatedSessionClaims {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub user_type: UserType,
    pub jti: Option<AccessTokenId>,
    pub exp: i64,
}
