//! Merge the bridge-owned V2 keys into the user's `opencode.json` and strip
//! what the V1 writer left behind, preserving every other key.
//!
//! V2 bridge-owned surface: `providers.systemprompt` (and `providers` once
//! empty), the top-level `model` when it names our provider, and the
//! `policies` statements [`provider_policies`] renders. V1 surface, stripped
//! on every write: `provider.systemprompt`, an `enabled_providers` that is
//! exactly our allowlist, the flat `mcp.<slug>` entries the sidecar recorded,
//! and the `auth.json` key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Map, Value};

use super::super::config::{DEFAULT_MODEL, POLICIES, PROVIDER_ID};
use super::profile::provider_policies;
use super::{pretty, read_object};

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn install(source: &Map<String, Value>, target: &Path) -> std::io::Result<()> {
    let mut merged = read_object(target)?;
    strip_v1(&mut merged, &recorded_v1_servers(target)?);
    strip_owned(&mut merged);
    if let Some(Value::Object(providers)) = source.get("providers") {
        let table = merged
            .entry("providers")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(table) = table {
            for (id, provider) in providers {
                table.insert(id.clone(), provider.clone());
            }
        }
    }
    if let Some(model) = source.get(DEFAULT_MODEL) {
        merged.insert(DEFAULT_MODEL.to_owned(), model.clone());
    }
    if let Some(Value::Array(ours)) = source.get(POLICIES) {
        let mut policies = ours.clone();
        if let Some(Value::Array(foreign)) = merged.remove(POLICIES) {
            policies.extend(foreign);
        }
        merged.insert(POLICIES.to_owned(), Value::Array(policies));
    }
    crate::fsutil::atomic_write_0600(target, &pretty(&merged)?)
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(super) fn uninstall(target: &Path) -> std::io::Result<bool> {
    if !target.exists() {
        return Ok(false);
    }
    let original = read_object(target)?;
    let mut value = original.clone();
    strip_v1(&mut value, &recorded_v1_servers(target)?);
    strip_owned(&mut value);
    if value == original {
        return Ok(false);
    }
    if value.is_empty() {
        crate::fsutil::remove_verified(target)?;
    } else {
        crate::fsutil::atomic_write_0600(target, &pretty(&value)?)?;
    }
    Ok(true)
}

fn recorded_v1_servers(target: &Path) -> std::io::Result<Vec<String>> {
    crate::integration::mcp_sidecar::read(&crate::integration::mcp_sidecar::beside(target))
        .map_err(std::io::Error::other)
}

// JSON: the bridge's own `policies` statements as they sit in a user-tier
// opencode.json, compared by value against what the file holds.
fn owned_policies() -> Vec<Value> {
    provider_policies()
        .iter()
        .filter_map(|policy| serde_json::to_value(policy).ok())
        .collect()
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(crate) fn strip_owned(root: &mut Map<String, Value>) {
    remove_table_entry(root, "providers", PROVIDER_ID);
    let ours = root
        .get(DEFAULT_MODEL)
        .and_then(Value::as_str)
        .is_some_and(|m| m.starts_with(&format!("{PROVIDER_ID}/")));
    if ours {
        root.remove(DEFAULT_MODEL);
    }
    let owned = owned_policies();
    if let Some(Value::Array(policies)) = root.get_mut(POLICIES) {
        policies.retain(|policy| !owned.contains(policy));
        if policies.is_empty() {
            root.remove(POLICIES);
        }
    }
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
pub(crate) fn strip_v1(root: &mut Map<String, Value>, recorded_servers: &[String]) {
    remove_table_entry(root, "provider", PROVIDER_ID);
    let allowlist_is_ours = root
        .get("enabled_providers")
        .and_then(Value::as_array)
        .is_some_and(|list| list.len() == 1 && list[0].as_str() == Some(PROVIDER_ID));
    if allowlist_is_ours {
        root.remove("enabled_providers");
    }
    if let Some(Value::Object(mcp)) = root.get_mut("mcp") {
        mcp.retain(|name, _| name == "servers" || !recorded_servers.contains(name));
        if mcp.is_empty() {
            root.remove("mcp");
        }
    }
}

// JSON: a user-tier opencode.json document, edited in place without a schema
// so keys the bridge does not own survive.
fn remove_table_entry(root: &mut Map<String, Value>, table: &str, key: &str) {
    if let Some(Value::Object(entries)) = root.get_mut(table) {
        entries.remove(key);
        if entries.is_empty() {
            root.remove(table);
        }
    }
}
