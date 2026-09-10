//! Plugin and plugin-rule identifiers.
//!
//! [`PluginRuleId`] names a rule a plugin ships as `rules/<id>.md`. It is
//! unrelated to `RuleId`, which names an authz rule row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(PluginId, schema);

crate::define_id!(PluginRuleId, schema);
