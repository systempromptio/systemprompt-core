//! Block destructive tool names for non-admin agents; registered as the
//! `tool_blocklist` policy.
//!
//! Configurable via:
//! ```yaml
//! - id: tool_blocklist
//!   patterns: ["delete", "drop", "destroy"]
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use serde::Deserialize;
use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::PolicyId;

use super::super::registry::{PolicyConfigurationError, PolicyRegistration};
use super::super::types::{AccessScope, GovernancePolicy, PolicyContext};
use crate::authz::types::{Decision, DenyReason, MatchedBy};

const ID: &str = "tool_blocklist";
const DEFAULT_PATTERNS: &[&str] = &["delete", "drop", "destroy"];

#[derive(Debug)]
struct ToolBlocklist {
    patterns: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolBlocklistYaml {
    #[serde(default)]
    patterns: Vec<String>,
}

impl ToolBlocklist {
    fn from_yaml(v: &YamlValue) -> Result<Self, PolicyConfigurationError> {
        let cfg = serde_yaml::from_value::<Option<ToolBlocklistYaml>>(v.clone())
            .map_err(|source| PolicyConfigurationError::Yaml {
                context: "malformed tool_blocklist policy entry",
                source,
            })?
            .unwrap_or_default();
        let patterns = if cfg.patterns.is_empty() {
            DEFAULT_PATTERNS.iter().map(|s| (*s).to_owned()).collect()
        } else {
            cfg.patterns
        };
        Ok(Self { patterns })
    }
}

impl GovernancePolicy for ToolBlocklist {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }
    fn name(&self) -> &'static str {
        "Tool Blocklist"
    }
    fn description(&self) -> &'static str {
        "Block tool names containing destructive substrings (e.g. delete/drop/destroy) \
         for any agent without admin scope."
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let allow = |detail| Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new(ID),
                detail,
            },
        };
        let Some(tool) = ctx.target.tool() else {
            return allow(Cow::Borrowed("Not a tool call"));
        };
        let matched = self
            .patterns
            .iter()
            .find(|p| tool.as_str().contains(p.as_str()));

        match matched {
            Some(p) if ctx.access_scope != AccessScope::Admin => Decision::Deny {
                reason: DenyReason::ToolBlocked {
                    tool: tool.clone(),
                    list_id: p.clone(),
                },
            },
            _ => allow(Cow::Borrowed("Tool not on restricted list")),
        }
    }
}

inventory::submit! {
    PolicyRegistration {
        id: ID,
        factory: |v| Ok(Box::new(ToolBlocklist::from_yaml(v)?)),
    }
}
