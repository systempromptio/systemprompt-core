//! Tenant identifier.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(TenantId, checked, |value| {
    crate::macros::validate_non_empty("TenantId", value)
});
