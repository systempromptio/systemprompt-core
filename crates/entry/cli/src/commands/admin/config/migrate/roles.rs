//! `access-control/roles.yaml` → `access-control/rules.yaml`.
//!
//! The pre-0.73 roles file listed one rule per `(entity, access)` with its
//! roles; 0.73 has one access-control dialect, the entity-centric
//! `rules.yaml`. This is the only code that still reads the old shape. Every
//! roles rule becomes an entity block (`entity_match` becomes the glob it
//! named), merged into an existing `rules.yaml`. It refuses rather than
//! guesses: a roles entity the rules file already declares, or a glob on a
//! kind that takes none, stops the migration naming the rule. The rules file
//! is written only after the merged document validates, and the roles file is
//! then removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use systemprompt_governance::authz::rules_file::{
    BandSpec, EntityDecl, EntityDefault, RETIRED_ROLES_FILE, RULES_FILE, RulesDoc, parse_rules_doc,
};

pub const ROLES_FILE: &str = RETIRED_ROLES_FILE;
const MIGRATED_WHY: &str = "Migrated from access-control/roles.yaml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RolesFile {
    #[serde(default)]
    rules: Vec<RoleRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleRule {
    entity_type: String,
    #[serde(default)]
    entity_id: Option<String>,
    #[serde(default)]
    entity_match: Option<String>,
    #[serde(default = "allow")]
    access: String,
    #[serde(default)]
    default_included: bool,
    #[serde(default)]
    roles: Vec<String>,
    #[serde(default)]
    justification: Option<String>,
}

fn allow() -> String {
    "allow".to_owned()
}

#[must_use]
pub fn pending(services: &Path) -> bool {
    services.join(ROLES_FILE).exists()
}

pub fn migrate(services: &Path) -> Result<Option<PathBuf>> {
    let roles_path = services.join(ROLES_FILE);
    if !roles_path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&roles_path)
        .with_context(|| format!("Failed to read {}", roles_path.display()))?;
    let roles: RolesFile = serde_yaml::from_str(&text)
        .with_context(|| format!("Failed to parse {}", roles_path.display()))?;
    let rules_path = services.join(RULES_FILE);
    let mut doc = match std::fs::read_to_string(&rules_path) {
        Ok(text) => parse_rules_doc(&text)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => RulesDoc::default(),
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to read {}", rules_path.display()));
        },
    };
    let converted = convert(&roles)?;
    for decl in converted {
        if doc.entities.iter().any(|d| d.entity == decl.entity) {
            bail!(
                "{ROLES_FILE} and {RULES_FILE} both declare {}; merge them by hand, then rerun",
                decl.entity
            );
        }
        doc.entities.push(decl);
    }
    doc.validate()?;
    let body = serde_yaml::to_string(&doc)?;
    std::fs::write(&rules_path, body)
        .with_context(|| format!("Failed to write {}", rules_path.display()))?;
    std::fs::remove_file(&roles_path)
        .with_context(|| format!("Failed to remove {}", roles_path.display()))?;
    Ok(Some(rules_path))
}

fn convert(roles: &RolesFile) -> Result<Vec<EntityDecl>> {
    let mut entities: BTreeMap<String, EntityDecl> = BTreeMap::new();
    for (index, rule) in roles.rules.iter().enumerate() {
        let ((Some(id), None) | (None, Some(id))) = (&rule.entity_id, &rule.entity_match) else {
            bail!("{ROLES_FILE} rules[{index}]: set exactly one of entity_id or entity_match");
        };
        let entity = format!("{}/{id}", rule.entity_type);
        let why = rule
            .justification
            .clone()
            .filter(|w| !w.trim().is_empty())
            .unwrap_or_else(|| MIGRATED_WHY.to_owned());
        let decl = entities
            .entry(entity.clone())
            .or_insert_with(|| EntityDecl {
                entity,
                default: EntityDefault::Closed,
                why: why.clone(),
                owner: None,
                allow: BTreeMap::new(),
                deny: BTreeMap::new(),
            });
        if rule.default_included {
            decl.default = EntityDefault::Open;
        }
        let bands = match rule.access.as_str() {
            "allow" => &mut decl.allow,
            "deny" => &mut decl.deny,
            other => bail!("{ROLES_FILE} rules[{index}]: access '{other}' is not allow or deny"),
        };
        let mut values: Vec<String> = bands
            .remove("role")
            .map(|spec| spec.values().to_vec())
            .unwrap_or_default();
        values.extend(rule.roles.iter().cloned());
        values.sort();
        values.dedup();
        let spec = if why == decl.why {
            BandSpec::List(values)
        } else {
            BandSpec::Detailed { values, why }
        };
        bands.insert("role".to_owned(), spec);
    }
    Ok(entities.into_values().collect())
}
