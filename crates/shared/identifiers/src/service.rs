//! Name of a platform-managed service — the `services.name` key shared by
//! MCP servers and agents, taken from the `services.yaml` key that declared
//! it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ServiceName, checked, |value| {
    crate::macros::validate_non_empty("ServiceName", value)
});
