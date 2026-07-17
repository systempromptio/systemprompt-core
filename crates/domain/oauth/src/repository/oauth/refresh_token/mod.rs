//! Refresh token persistence and rotation. Token identifiers are stored as
//! HMAC-SHA-256 digests under the deployment pepper; raw `RefreshTokenId`
//! values never touch the database. Consumed tokens are retained as
//! tombstones (`consumed_at IS NOT NULL`) so a replay can be distinguished
//! from "token never existed" and trigger family-wide revocation per
//! RFC 6819 §5.2.2.3.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ops;

use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};

#[derive(Debug)]
pub struct RefreshTokenParams<'a> {
    pub token_id: &'a RefreshTokenId,
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: &'a str,
    pub expires_at: i64,
    /// Family-identifier shared by every refresh token derived from the same
    /// initial authorization-code exchange. When `None`, the family is seeded
    /// from `token_id` (first issuance). Subsequent rotations carry the parent
    /// token's family forward so a single auth-code-replay or refresh-token-
    /// replay detection can invalidate every descendant.
    pub family_id: Option<&'a str>,
}

#[derive(Debug)]
pub struct RefreshTokenParamsBuilder<'a> {
    token_id: &'a RefreshTokenId,
    client_id: &'a ClientId,
    user_id: &'a UserId,
    scope: &'a str,
    expires_at: i64,
    family_id: Option<&'a str>,
}

impl<'a> RefreshTokenParamsBuilder<'a> {
    pub const fn new(
        token_id: &'a RefreshTokenId,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        scope: &'a str,
        expires_at: i64,
    ) -> Self {
        Self {
            token_id,
            client_id,
            user_id,
            scope,
            expires_at,
            family_id: None,
        }
    }

    pub const fn with_family(mut self, family_id: &'a str) -> Self {
        self.family_id = Some(family_id);
        self
    }

    pub const fn build(self) -> RefreshTokenParams<'a> {
        RefreshTokenParams {
            token_id: self.token_id,
            client_id: self.client_id,
            user_id: self.user_id,
            scope: self.scope,
            expires_at: self.expires_at,
            family_id: self.family_id,
        }
    }
}

impl<'a> RefreshTokenParams<'a> {
    pub const fn builder(
        token_id: &'a RefreshTokenId,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        scope: &'a str,
        expires_at: i64,
    ) -> RefreshTokenParamsBuilder<'a> {
        RefreshTokenParamsBuilder::new(token_id, client_id, user_id, scope, expires_at)
    }
}

#[derive(Debug)]
pub struct ConsumedRefreshToken {
    pub user_id: UserId,
    pub scope: String,
    pub family_id: String,
}
