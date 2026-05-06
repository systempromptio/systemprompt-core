//! YAML schema for declarative access-control baselines.
//!
//! A deployment commits an [`AccessControlConfig`] (typically at
//! `services/access-control/*.yaml` or under `services/governance/`) that
//! declares the role- and department-level rules every instance should boot
//! with. The bootstrap loader (in `systemprompt-sync`) parses this struct,
//! hands it to [`super::ingestion::AccessControlIngestionService`], and the
//! service projects it into `access_control_rules`.
//!
//! The contract is one-way (YAML → DB). Per-user overrides
//! (`rule_type='user'`) are operational state and never appear in this
//! schema — the loader rejects any rule that has neither `roles:` nor
//! `departments:`.
//!
//! `departments[]` is declarative metadata: the loader validates that every
//! department referenced by a rule appears in this list (typo guard) but
//! does not persist the entries — there is no `departments` table.

use serde::{Deserialize, Serialize};

use super::error::AuthzError;
use super::types::{Access, EntityKind};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessControlConfig {
    #[serde(default)]
    pub departments: Vec<DepartmentEntry>,
    #[serde(default)]
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepartmentEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleEntry {
    pub entity_type: EntityKind,
    pub entity_id: String,
    pub access: Access,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub departments: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

impl AccessControlConfig {
    pub fn validate(&self) -> Result<(), AuthzError> {
        let mut problems: Vec<String> = Vec::new();

        let mut declared: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(self.departments.len());
        for (idx, dept) in self.departments.iter().enumerate() {
            if dept.name.trim().is_empty() {
                problems.push(format!("departments[{idx}]: name is empty"));
                continue;
            }
            if !declared.insert(dept.name.as_str()) {
                problems.push(format!(
                    "departments[{idx}]: duplicate department name '{}'",
                    dept.name
                ));
            }
        }

        for (idx, rule) in self.rules.iter().enumerate() {
            if rule.entity_id.trim().is_empty() {
                problems.push(format!("rules[{idx}]: entity_id is empty"));
            }
            if rule.roles.is_empty() && rule.departments.is_empty() {
                problems.push(format!(
                    "rules[{idx}]: must declare at least one of roles[] or departments[] — \
                     per-user rules belong to runtime state, not YAML"
                ));
            }
            for dept in &rule.departments {
                if !declared.contains(dept.as_str()) {
                    problems.push(format!(
                        "rules[{idx}]: references undeclared department '{dept}' (add it to the \
                         top-level departments: list)"
                    ));
                }
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
