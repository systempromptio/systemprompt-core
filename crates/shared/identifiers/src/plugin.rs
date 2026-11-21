//! Plugin identifiers: the plugin's catalog key, carried in manifests,
//! plugin-scoped tokens and authz entity refs, and the directory name of the
//! plugin under the services tree, so it is checked as one path segment: no
//! separator, no null byte, never `.` or `..`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(PluginId, checked, |value| {
    crate::macros::validate_path_segment("PluginId", value)
});
