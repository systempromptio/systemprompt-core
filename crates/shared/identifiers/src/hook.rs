//! Extension-hook identifier.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(HookId, checked, |value| {
    crate::macros::validate_non_empty("HookId", value)
});

impl HookId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
