//! The marketplace `access:` assignment block and its validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_models::errors::ServicesValidationError;

use super::marketplace::MarketplaceAccessRule;

fn is_extension_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.starts_with('_')
        && !slug.ends_with('_')
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Declarative assignment block for a marketplace.
///
/// `roles` and `rules` are the identity vectors core inspects: role strings are
/// matched against `access_control_rules` for the core RBAC check, mirroring
/// [`JwtClaims::roles`](systemprompt_models::auth::JwtClaims), and each
/// [`MarketplaceAccessRule`] projects one further subject dimension (group,
/// project, department, …) into the same table. `attributes` is an opaque,
/// dotted-namespace bag core never interprets — it is forwarded verbatim to
/// extension authz/ABAC hooks, exactly
/// as [`JwtClaims::attributes`](systemprompt_models::auth::JwtClaims) is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct MarketplaceAccess {
    #[serde(default)]
    pub default_included: bool,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<MarketplaceAccessRule>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    // JSON: ABAC attribute values are declared per deployment in the authz policy YAML.
    pub attributes: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

impl MarketplaceAccess {
    #[must_use]
    pub const fn declares_rules(&self) -> bool {
        !self.roles.is_empty() || !self.rules.is_empty()
    }

    #[must_use]
    pub fn is_declared(&self) -> bool {
        self.default_included
            || self.declares_rules()
            || !self.attributes.is_empty()
            || self.justification.is_some()
    }

    #[must_use]
    pub fn rule_types(&self) -> BTreeSet<&str> {
        let mut out: BTreeSet<&str> = self.rules.iter().map(|r| r.rule_type.as_str()).collect();
        if !self.roles.is_empty() {
            out.insert("role");
        }
        out
    }

    pub(super) fn validate(&self, key: &str) -> Result<(), ServicesValidationError> {
        if self.roles.iter().any(|role| role.trim().is_empty()) {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': access.roles must not contain blank entries"
            )));
        }

        for rule in &self.rules {
            let slug = rule.rule_type.as_str();
            if slug == "role" || slug == "user" {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': access.rules may not use rule_type '{slug}' — declare \
                     roles under access.roles"
                )));
            }
            if !is_extension_slug(slug) {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': access.rules rule_type '{slug}' must be lowercase \
                     alphanumeric with underscores, and may not start or end with '_'"
                )));
            }
            if rule.values.is_empty() {
                return Err(ServicesValidationError::required(format!(
                    "Marketplace '{key}': access.rules entry '{slug}' must name at least one value"
                )));
            }
            if rule.values.iter().any(|value| value.trim().is_empty()) {
                return Err(ServicesValidationError::invalid_field(format!(
                    "Marketplace '{key}': access.rules entry '{slug}' must not contain blank values"
                )));
            }
        }

        Ok(())
    }
}
