//! YAML schema for declarative access-control baselines.
//!
//! A deployment commits an [`AccessControlConfig`] (typically at
//! `services/access-control/*.yaml` or under `services/governance/`) that
//! declares the role-level rules every instance should boot with. The
//! bootstrap loader (in `systemprompt-sync`) parses this struct, hands it
//! to [`super::ingestion::AccessControlIngestionService`], and the service
//! projects it into `access_control_rules`.
//!
//! The contract is one-way (YAML → DB). Per-user overrides
//! (`rule_type='user'`) are operational state and never appear in this
//! schema — the loader rejects any rule that has no `roles:` set.
//!
//! Per-tenant attribute-based rules (department, clearance, jurisdiction,
//! ...) are NOT modelled here: they live in extension-owned tables and
//! are evaluated by an extension `AuthzDecisionHook` composed alongside
//! the core resolver via [`super::CompositeAuthzHook`].

use serde::{Deserialize, Serialize};

use super::error::AuthzError;
use super::types::{Access, EntityKind};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessControlConfig {
    #[serde(default)]
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleEntry {
    pub entity_type: EntityKind,
    pub entity_id: String,
    pub access: Access,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

impl AccessControlConfig {
    pub fn validate(&self) -> Result<(), AuthzError> {
        let mut problems: Vec<String> = Vec::new();

        for (idx, rule) in self.rules.iter().enumerate() {
            if rule.entity_id.trim().is_empty() {
                problems.push(format!("rules[{idx}]: entity_id is empty"));
            }
            if rule.roles.is_empty() {
                problems.push(format!(
                    "rules[{idx}]: must declare at least one role — per-user rules belong to \
                     runtime state, not YAML, and attribute-based rules belong in an extension \
                     hook"
                ));
            }
            for role in &rule.roles {
                if role.trim().is_empty() {
                    problems.push(format!("rules[{idx}]: empty role string"));
                }
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(AuthzError::Validation(problems.join("; ")))
        }
    }
}
