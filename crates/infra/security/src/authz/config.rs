//! YAML schema for declarative access-control baselines.
//!
//! A deployment commits an [`AccessControlConfig`] (typically at
//! `services/access-control/*.yaml`) that declares the role-level rules every
//! instance should boot with. A loader parses this struct, hands it to
//! [`super::ingestion::AccessControlIngestionService`], and the service
//! projects it into `access_control_entities` + `access_control_rules`.
//!
//! Each rule names its subject set in exactly one of two ways:
//!
//! - `entity_id` — a literal catalog id; the loader self-materialises the
//!   entity row, so the grant survives a clean install even if nothing else
//!   registered the entity yet.
//! - `entity_match` — a `*`-glob expanded against the entities already present
//!   in the catalog for that [`EntityKind`]; one rule per matched id. The glob
//!   never creates entities — it only grants ones a prior pass materialised.
//!
//! The contract is one-way (YAML → DB). Per-user overrides (`rule_type='user'`)
//! are operational state and never appear here — the loader rejects any rule
//! with no `roles:` set. Per-tenant attribute rules live in extension-owned
//! tables and are evaluated by an extension `AuthzDecisionHook`.

use serde::{Deserialize, Serialize, Serializer};

use super::error::AuthzError;
use super::types::{Access, EntityKind};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessControlConfig {
    #[serde(default)]
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone)]
pub enum RuleTarget {
    Id(String),
    Match(String),
}

#[derive(Debug, Clone)]
pub struct RuleEntry {
    pub entity_type: EntityKind,
    pub target: RuleTarget,
    pub access: Access,
    pub default_included: bool,
    pub roles: Vec<String>,
    pub justification: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleEntryWire {
    entity_type: EntityKind,
    #[serde(default)]
    entity_id: Option<String>,
    #[serde(default)]
    entity_match: Option<String>,
    #[serde(default = "default_allow")]
    access: Access,
    #[serde(default)]
    default_included: bool,
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    justification: Option<String>,
}

const fn default_allow() -> Access {
    Access::Allow
}

#[derive(Serialize)]
struct RuleEntryOut<'a> {
    entity_type: EntityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_match: Option<&'a str>,
    access: Access,
    default_included: bool,
    roles: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    justification: Option<&'a str>,
}

impl Serialize for RuleEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (entity_id, entity_match) = match &self.target {
            RuleTarget::Id(id) => (Some(id.as_str()), None),
            RuleTarget::Match(pattern) => (None, Some(pattern.as_str())),
        };
        RuleEntryOut {
            entity_type: self.entity_type,
            entity_id,
            entity_match,
            access: self.access,
            default_included: self.default_included,
            roles: &self.roles,
            justification: self.justification.as_deref(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuleEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RuleEntryWire::deserialize(deserializer)?;
        let target = match (wire.entity_id, wire.entity_match) {
            (Some(id), None) => RuleTarget::Id(id),
            (None, Some(pattern)) => RuleTarget::Match(pattern),
            (Some(_), Some(_)) => {
                return Err(serde::de::Error::custom(format!(
                    "rule for entity_type={} sets both entity_id and entity_match; pick one",
                    wire.entity_type.as_str()
                )));
            },
            (None, None) => {
                return Err(serde::de::Error::custom(format!(
                    "rule for entity_type={} sets neither entity_id nor entity_match",
                    wire.entity_type.as_str()
                )));
            },
        };
        Ok(Self {
            entity_type: wire.entity_type,
            target,
            access: wire.access,
            default_included: wire.default_included,
            roles: wire.roles,
            justification: wire.justification,
        })
    }
}

impl AccessControlConfig {
    pub fn validate(&self) -> Result<(), AuthzError> {
        let mut problems: Vec<String> = Vec::new();

        for (idx, rule) in self.rules.iter().enumerate() {
            match &rule.target {
                RuleTarget::Id(id) if id.trim().is_empty() => {
                    problems.push(format!("rules[{idx}]: entity_id is empty"));
                },
                RuleTarget::Match(pattern) if pattern.trim().is_empty() => {
                    problems.push(format!("rules[{idx}]: entity_match is empty"));
                },
                _ => {},
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
