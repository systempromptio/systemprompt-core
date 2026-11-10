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

use systemprompt_identifiers::{
    AuthorizationCode, ClientId, RefreshTokenFamilyId, RefreshTokenId, UserId,
};

#[derive(Debug)]
pub struct RefreshTokenParams<'a> {
    pub token_id: &'a RefreshTokenId,
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: &'a str,
    pub expires_at: i64,
    pub resource: Option<&'a str>,
    pub lineage: RefreshTokenLineage<'a>,
}

/// Where a stored refresh token comes from.
///
/// A token minted by redeeming an authorization code starts its own family and
/// is stored only while that code still names it; a rotated token joins the
/// family it replaces.
#[derive(Debug, Clone, Copy)]
pub enum RefreshTokenLineage<'a> {
    AuthorizationCode(&'a AuthorizationCode),
    Family(&'a RefreshTokenFamilyId),
}

#[derive(Debug)]
pub struct ConsumedRefreshToken {
    pub user_id: UserId,
    pub scope: String,
    pub family_id: RefreshTokenFamilyId,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenHolder {
    pub client_id: ClientId,
    pub user_id: UserId,
}
