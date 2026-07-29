//! `scope_check`: gate admin-only tools by [`AccessScope`].
//!
//! Reads the typed `ctx.access_scope` the enforcement point resolved (agent
//! YAML `oauth.scopes`, JWT permissions, DB roles). Configurable via:
//!
//! ```yaml
//! - id: scope_check
//!   admin_only_prefixes:
//!     - "mcp__systemprompt__"
//! ```

use std::borrow::Cow;

use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::PolicyId;

use super::super::registry::PolicyRegistration;
use super::super::types::{AccessScope, GovernancePolicy, PolicyContext};
use crate::authz::types::{Decision, DenyReason, MatchedBy};

const ID: &str = "scope_check";
const DEFAULT_ADMIN_ONLY_PREFIXES: &[&str] = &["mcp__systemprompt__"];

#[derive(Debug)]
struct ScopeCheck {
    admin_only_prefixes: Vec<String>,
}

impl ScopeCheck {
    fn from_yaml(v: &YamlValue) -> Self {
        let prefixes = v
            .get("admin_only_prefixes")
            .and_then(|s| s.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|p| p.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .filter(|v: &Vec<String>| !v.is_empty())
            .unwrap_or_else(|| {
                DEFAULT_ADMIN_ONLY_PREFIXES
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect()
            });
        Self {
            admin_only_prefixes: prefixes,
        }
    }
}

impl GovernancePolicy for ScopeCheck {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }
    fn name(&self) -> &'static str {
        "Scope Check"
    }
    fn description(&self) -> &'static str {
        "Block non-admin agents from calling tools whose name starts with an \
         admin-only prefix (default: mcp__systemprompt__)."
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        if ctx.access_scope == AccessScope::Admin {
            return Decision::Allow {
                matched_by: MatchedBy::PolicyAllow {
                    policy_id: PolicyId::new(ID),
                    detail: Cow::Borrowed("admin scope grants unrestricted tool access"),
                },
            };
        }

        let Some(tool) = ctx.target.tool() else {
            return Decision::Allow {
                matched_by: MatchedBy::PolicyAllow {
                    policy_id: PolicyId::new(ID),
                    detail: Cow::Borrowed("Not a tool call"),
                },
            };
        };
        let tool_str = tool.as_str();
        let requires_admin = self
            .admin_only_prefixes
            .iter()
            .any(|prefix| tool_str.starts_with(prefix.as_str()));

        if requires_admin {
            return Decision::Deny {
                reason: DenyReason::ScopeViolation {
                    tool: tool.clone(),
                    required: AccessScope::Admin,
                },
            };
        }

        let detail = match ctx.access_scope {
            AccessScope::Unknown => {
                Cow::Borrowed("Agent scope could not be resolved; allowed for non-admin tool")
            },
            scope => Cow::Owned(format!("{scope} scope is allowed for tool: {tool_str}")),
        };
        Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new(ID),
                detail,
            },
        }
    }
}

inventory::submit! {
    PolicyRegistration {
        id: ID,
        factory: |v| Box::new(ScopeCheck::from_yaml(v)),
    }
}
