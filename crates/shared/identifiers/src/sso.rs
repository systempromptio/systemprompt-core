//! Single sign-on identifiers.
//!
//! [`SsoRequestId`] is the protocol request id a provider correlates between
//! `start` and `finish` (the SAML `AuthnRequest` id). [`SsoAssertionId`] is
//! the id the identity provider stamps on the assertion it returns, and
//! [`IdpTenantId`] the identity provider's own tenant, which is not a
//! systemprompt [`crate::TenantId`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(SsoRequestId, checked, |value| {
    crate::macros::validate_non_empty("SsoRequestId", value)
});
crate::define_id!(SsoAssertionId, checked, |value| {
    crate::macros::validate_non_empty("SsoAssertionId", value)
});
crate::define_id!(IdpTenantId, checked, |value| {
    crate::macros::validate_non_empty("IdpTenantId", value)
});
