//! Plugin identifiers: the plugin's catalog key, a checked non-empty slug
//! carried in manifests, plugin-scoped tokens and authz entity refs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(PluginId, checked, |value| {
    crate::macros::validate_non_empty("PluginId", value)
});
