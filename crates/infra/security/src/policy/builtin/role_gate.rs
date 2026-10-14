//! Role-scoped tool rules shared by `scope_check` and `tool_blocklist`.
//!
//! A rule names a tool prefix, optionally the substrings a tool under it must
//! contain, and the roles that pass it. `scope_check` reads it as "these tools
//! need one of these roles (or admin)"; `tool_blocklist` reads it as "a
//! blocked tool under this prefix is allowed to these roles".
//!
//! ```yaml
//! - prefix: "mcp__odoo__"
//!   contains: ["delete", "anonymise", "erase"]
//!   roles: ["odoo_destructive"]
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoleRule {
    pub(super) prefix: String,
    #[serde(default)]
    pub(super) contains: Vec<String>,
    pub(super) roles: Vec<String>,
}

impl RoleRule {
    pub(super) fn covers(&self, tool: &str) -> bool {
        tool.starts_with(self.prefix.as_str())
            && (self.contains.is_empty() || self.contains.iter().any(|c| tool.contains(c.as_str())))
    }

    pub(super) fn held_by<'r>(&self, roles: &'r [String]) -> Option<&'r str> {
        roles
            .iter()
            .find(|role| self.roles.contains(role))
            .map(String::as_str)
    }
}
