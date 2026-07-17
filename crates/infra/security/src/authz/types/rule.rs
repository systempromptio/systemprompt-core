//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::RuleId;

use super::kinds::{Access, EntityKind, RuleType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccessRule {
    pub id: RuleId,
    pub rule_type: RuleType,
    pub rule_value: String,
    pub access: Access,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

/// One row from `access_control_entities`.
///
/// The `source` provenance string identifies which loader pass first
/// registered the entity: `"profile:<name>"`, `"roles.yaml"`, or
/// `"bootstrap:*"` for rows promoted from older schemas by a migration.
///
/// A `None` lookup result means the entity is unknown to access control and
/// the resolver returns [`super::decision::DenyReason::UnknownEntity`] rather
/// than the generic `NotAssigned`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRow {
    pub kind: EntityKind,
    pub id: String,
    pub default_included: bool,
    pub source: String,
}
