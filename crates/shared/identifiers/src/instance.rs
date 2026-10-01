//! Replica identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(InstanceId, checked, |value| {
    crate::macros::validate_non_empty("InstanceId", value)
});
