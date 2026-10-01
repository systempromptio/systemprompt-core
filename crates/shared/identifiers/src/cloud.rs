//! Identifiers for cloud billing and account surfaces.
//!
//! `CloudUserId` is the account id the systemprompt.io cloud management API
//! assigns. It is an external-protocol value, not a local `users.id`, so it
//! is a checked opaque string rather than a `UserId`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(PriceId, schema);
crate::define_id!(CloudUserId, checked, |value| {
    crate::macros::validate_non_empty("CloudUserId", value)
});
