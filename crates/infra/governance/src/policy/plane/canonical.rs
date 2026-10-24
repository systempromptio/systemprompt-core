//! The governance chain as plane entities and back.
//!
//! `services/governance/config.yaml` becomes one `governance` entity (the
//! chain's own keys: `enabled`, `mode`) and one `policy/<id>` entity per
//! policy entry carrying its position in the chain. Values are JSON with
//! every object's keys sorted, so formatting and key order in the file never
//! read as a change. [`config_from_entities`] reassembles the document and
//! parses it with [`GovernanceConfig::parse`], so a chain read from the
//! database is held to exactly the checks the file is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use serde_yaml::Value as YamlValue;

use crate::policy::config::{GovernanceConfig, GovernanceConfigError};

pub const CHAIN_ENTITY: &str = "governance";
pub const POLICY_PREFIX: &str = "policy/";

#[derive(Debug, Serialize, Deserialize)]
struct PolicyEntity {
    position: usize,
    // JSON: one `governance.policies` entry, keys sorted; its shape is the policy's.
    entry: Value,
}

// JSON: re-orders every object so equal declarations print equally.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, sorted(v)))
                    .collect::<Map<_, _>>(),
            )
        },
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

// JSON: a governance YAML node as sorted JSON; its shape is the declaring
// policy's.
fn to_json(yaml: &YamlValue) -> Result<Value, GovernanceConfigError> {
    serde_json::to_value(yaml)
        .map(sorted)
        .map_err(GovernanceConfigError::Canonical)
}

pub fn declared_entities(text: &str) -> Result<BTreeMap<String, String>, GovernanceConfigError> {
    GovernanceConfig::parse(text)?;
    let root: YamlValue = serde_yaml::from_str(text)?;
    let chain = root
        .get("governance")
        .and_then(YamlValue::as_mapping)
        .ok_or(GovernanceConfigError::MissingPolicies)?;
    let mut settings = chain.clone();
    settings.remove("policies");
    let mut out = BTreeMap::new();
    out.insert(
        CHAIN_ENTITY.to_owned(),
        to_json(&YamlValue::Mapping(settings))?.to_string(),
    );
    let policies = chain
        .get("policies")
        .and_then(YamlValue::as_sequence)
        .ok_or(GovernanceConfigError::MissingPolicies)?;
    for (position, entry) in policies.iter().enumerate() {
        let id = entry
            .get("id")
            .and_then(YamlValue::as_str)
            .ok_or(GovernanceConfigError::MissingPolicyId { index: position })?;
        let value = PolicyEntity {
            position,
            entry: to_json(entry)?,
        };
        let encoded = serde_json::to_string(&value).map_err(GovernanceConfigError::Canonical)?;
        if out
            .insert(format!("{POLICY_PREFIX}{id}"), encoded)
            .is_some()
        {
            return Err(GovernanceConfigError::DuplicatePolicyId { id: id.to_owned() });
        }
    }
    Ok(out)
}

pub(super) fn render_document(
    entities: &BTreeMap<String, String>,
) -> Result<String, GovernanceConfigError> {
    let mut chain: Map<String, Value> = match entities.get(CHAIN_ENTITY) {
        Some(value) => serde_json::from_str(value).map_err(GovernanceConfigError::Canonical)?,
        None => Map::new(),
    };
    let mut policies: Vec<PolicyEntity> = entities
        .iter()
        .filter(|(key, _)| key.starts_with(POLICY_PREFIX))
        .map(|(_, value)| serde_json::from_str(value).map_err(GovernanceConfigError::Canonical))
        .collect::<Result<_, _>>()?;
    policies.sort_by_key(|p| p.position);
    chain.insert(
        "policies".to_owned(),
        Value::Array(policies.into_iter().map(|p| p.entry).collect()),
    );
    let mut root = Map::new();
    root.insert("governance".to_owned(), Value::Object(chain));
    Ok(serde_yaml::to_string(&Value::Object(root))?)
}

pub fn config_from_entities(
    entities: &BTreeMap<String, String>,
) -> Result<Option<GovernanceConfig>, GovernanceConfigError> {
    if entities.is_empty() {
        return Ok(None);
    }
    GovernanceConfig::parse(&render_document(entities)?).map(Some)
}
