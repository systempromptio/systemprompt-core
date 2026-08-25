//! Local JWT claim decoding for identity display (no signature verification).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{VerifiedIdentity, now_unix};

pub fn decode_jwt_identity_unverified(token: &str) -> Option<VerifiedIdentity> {
    let claims = crate::auth::jwt::decode_unverified(token)?;
    Some(VerifiedIdentity {
        email: claims.email,
        user_id: claims.user_id,
        tenant_id: claims.tenant_id,
        exp_unix: claims.exp,
        verified_at_unix: now_unix(),
    })
}
