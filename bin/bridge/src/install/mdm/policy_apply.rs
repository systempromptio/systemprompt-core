//! Applying a rendered Claude Desktop policy to one hive so that a key the
//! policy withholds is removed rather than left at its last written value.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::store::verified::{self, PolicyReceipt};
use crate::config::store::{ConfigStore, ConfigStoreError, PolicyHive, PolicyTarget};

pub type RenderedValues = [(&'static str, &'static str, String)];

/// Keys the policy may withhold. A withheld key is cleared from the hive: a
/// value an older install left there (a `managedMcpServers` list pointing at
/// another loopback port) would otherwise stay in force.
pub const RECONCILED_KEYS: &[&str] = &["managedMcpServers"];

#[must_use]
pub fn withheld_keys(values: &RenderedValues) -> Vec<&'static str> {
    withheld_among(values.iter().map(|(name, _, _)| *name))
}

fn withheld_among<'a>(names: impl Iterator<Item = &'a str> + Clone) -> Vec<&'static str> {
    RECONCILED_KEYS
        .iter()
        .copied()
        .filter(|key| !names.clone().any(|name| name == *key))
        .collect()
}

pub fn apply_claude_policy(
    store: &dyn ConfigStore,
    hive: PolicyHive,
    values: &RenderedValues,
) -> Result<PolicyReceipt, ConfigStoreError> {
    let entries: Vec<(String, String)> = values
        .iter()
        .map(|(name, _, value)| ((*name).to_owned(), value.clone()))
        .collect();
    apply_claude_entries(store, hive, &entries)
}

pub fn apply_claude_entries(
    store: &dyn ConfigStore,
    hive: PolicyHive,
    entries: &[(String, String)],
) -> Result<PolicyReceipt, ConfigStoreError> {
    let receipt = verified::apply(
        store,
        hive,
        PolicyTarget::Claude,
        &crate::config::store::PolicyDocumentValue::strings(entries),
    )?;
    let withheld = withheld_among(entries.iter().map(|(name, _)| name.as_str()));
    if !withheld.is_empty() {
        let removed = verified::remove_values(store, hive, PolicyTarget::Claude, &withheld)?;
        if removed > 0 {
            tracing::info!(
                target: "bridge::mdm",
                hive = hive.label(),
                keys = ?withheld,
                "stale policy values the current policy withholds were removed"
            );
        }
    }
    Ok(receipt)
}
