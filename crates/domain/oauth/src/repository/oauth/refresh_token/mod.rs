//! Refresh token persistence and rotation. Token identifiers are stored as
//! HMAC-SHA-256 digests under the deployment pepper; raw `RefreshTokenId`
//! values never touch the database. Consumed tokens are retained as
//! tombstones (`consumed_at IS NOT NULL`) so a replay can be distinguished
//! from "token never existed" and trigger family-wide revocation per
//! RFC 6819 §5.2.2.3. `authenticated_at` is when the person signed in for the
//! grant and `session_started_at` is when the authorization code the family
//! descends from was issued; a rotated token is stored with the values its
//! predecessor carried. A token that descends from a device link's code
//! carries that link's `device_key_id`, taken from the code on redemption and
//! from the family on rotation, never from the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ops;

use chrono::{DateTime, Utc};
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
    pub authenticated_at: DateTime<Utc>,
    pub session_started_at: DateTime<Utc>,
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
    pub authenticated_at: DateTime<Utc>,
    pub session_started_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenHolder {
    pub client_id: ClientId,
    pub user_id: UserId,
}
