//! OAuth flow identifiers.
//!
//! Refresh tokens, access-token `jti`s and authorization codes are opaque
//! strings minted by the platform or presented back by a client. Their only
//! shape is non-emptiness: `new` is for a value the platform minted or read
//! back from storage, `try_new` for a value arriving from outside. A refresh
//! token family groups every rotation of one grant, and an SSO provider id
//! names a configured identity provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(RefreshTokenId, checked, |value| {
    crate::macros::validate_non_empty("RefreshTokenId", value)
});
crate::define_id!(AccessTokenId, checked, |value| {
    crate::macros::validate_non_empty("AccessTokenId", value)
});
crate::define_id!(AuthorizationCode, checked, |value| {
    crate::macros::validate_non_empty("AuthorizationCode", value)
});
crate::define_id!(ChallengeId);
crate::define_id!(RefreshTokenFamilyId);
crate::define_id!(SsoProviderId);

impl AccessTokenId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl ChallengeId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
