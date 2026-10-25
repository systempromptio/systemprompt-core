//! A marketplace's `access:` block as a declared entity of the access-control
//! plane.
//!
//! The block is the older spelling of what `rules.yaml` writes as a
//! `marketplace/<id>` entity, and it is read here into the same
//! [`CanonicalEntity`] so there is one dialect and one writer: `config apply`.
//! `access.roles` become `allow` rows in the `role` band and each
//! `access.rules` entry becomes one row per value in the band it names. A
//! block that declares nothing contributes no entity, so a marketplace without
//! one keeps whatever the plane set. The entity's origin is the marketplace's
//! own config file, `code:marketplaces/<id>/config.yaml`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use systemprompt_manifest::services::{MarketplaceAccess, MarketplaceConfigFile};

use super::{CanonicalEntity, CanonicalRule, DeclaredAccess};
use crate::authz::error::{AuthzError, AuthzResult};
use crate::authz::types::{Access, EntityKind, RuleType};

const MARKETPLACES_DIR: &str = "marketplaces";
const CONFIG_FILE: &str = "config.yaml";
const DEFAULT_WHY: &str = "declared by the marketplace's access block";

pub fn declared_marketplace_entities(
    services_root: &Path,
) -> AuthzResult<BTreeMap<String, DeclaredAccess>> {
    let dir = services_root.join(MARKETPLACES_DIR);
    let mut out = BTreeMap::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(source) => return Err(file_error("read", &dir, source)),
    };
    for entry in entries {
        let entry = entry.map_err(|source| file_error("read", &dir, source))?;
        let path = entry.path().join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => return Err(file_error("read", &path, source)),
        };
        let file: MarketplaceConfigFile =
            serde_yaml::from_str(&text).map_err(|source| AuthzError::ConfigParse {
                path: path.display().to_string(),
                source,
            })?;
        let marketplace = file.marketplace;
        if !marketplace.access.is_declared() {
            continue;
        }
        let id = marketplace.id.as_str();
        let canonical = canonical_access(id, &marketplace.access)?;
        out.insert(
            format!("{}/{id}", EntityKind::Marketplace.as_str()),
            DeclaredAccess {
                canonical,
                owner_bundle: None,
                file: format!(
                    "{MARKETPLACES_DIR}/{}/{CONFIG_FILE}",
                    entry.file_name().to_string_lossy()
                ),
            },
        );
    }
    Ok(out)
}

fn canonical_access(id: &str, access: &MarketplaceAccess) -> AuthzResult<CanonicalEntity> {
    let why = access.justification.as_deref().unwrap_or(DEFAULT_WHY);
    let mut rules = BTreeSet::new();
    for role in &access.roles {
        if role.trim().is_empty() {
            return Err(AuthzError::Validation(format!(
                "marketplace '{id}': access.roles must not contain blank entries"
            )));
        }
        rules.insert(CanonicalRule {
            rule_type: RuleType::ROLE.to_string(),
            rule_value: role.clone(),
            access: Access::Allow,
            why: why.to_owned(),
        });
    }
    for rule in &access.rules {
        let rule_type = RuleType::extension(rule.rule_type.clone()).map_err(|source| {
            AuthzError::MarketplaceRuleType {
                marketplace: id.to_owned(),
                rule_type: rule.rule_type.clone(),
                source: Box::new(source),
            }
        })?;
        let rule_access = match rule.access.as_str() {
            "deny" => Access::Deny,
            _ => Access::Allow,
        };
        let rule_why = rule.justification.as_deref().unwrap_or(why);
        for value in &rule.values {
            rules.insert(CanonicalRule {
                rule_type: rule_type.to_string(),
                rule_value: value.clone(),
                access: rule_access,
                why: rule_why.to_owned(),
            });
        }
    }
    Ok(CanonicalEntity {
        default_included: access.default_included,
        rules,
    })
}

fn file_error(action: &'static str, path: &Path, source: std::io::Error) -> AuthzError {
    AuthzError::File {
        action,
        path: path.display().to_string(),
        source,
    }
}
