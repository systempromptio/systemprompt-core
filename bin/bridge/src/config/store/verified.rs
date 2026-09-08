//! Verified policy operations over an injected platform store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ConfigStore, ConfigStoreError, PolicyDocumentValue, PolicyHive, PolicyTarget, PolicyWrite,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[must_use]
pub struct PolicyReceipt {
    outcome: PolicyWrite,
    names: Vec<String>,
    location: String,
}

impl PolicyReceipt {
    #[cfg(target_os = "windows")]
    pub(crate) const fn new(outcome: PolicyWrite, location: String, names: Vec<String>) -> Self {
        Self {
            outcome,
            names,
            location,
        }
    }
    pub fn describe(&self) -> String {
        format!(
            "{:?}: {} [{}]",
            self.outcome,
            self.location,
            self.names.join(", ")
        )
    }

    #[must_use]
    pub const fn outcome(&self) -> PolicyWrite {
        self.outcome
    }

    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

pub fn apply(
    store: &dyn ConfigStore,
    hive: PolicyHive,
    target: PolicyTarget,
    entries: &[(String, PolicyDocumentValue)],
) -> Result<PolicyReceipt, ConfigStoreError> {
    let subkey = target.subkey();
    let names: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    let authoritative =
        if hive == PolicyHive::User && store.policy_key_exists(PolicyHive::Machine, target)? {
            PolicyHive::Machine
        } else {
            hive
        };
    let current = store.read_policy_document(authoritative, target, &names)?;
    let differing: Vec<String> = entries
        .iter()
        .filter(|(name, value)| current.get(name) != Some(value))
        .map(|(name, _)| name.clone())
        .collect();
    if authoritative != hive {
        if !differing.is_empty() {
            return Err(ConfigStoreError::HiveConflict { subkey, differing });
        }
        return Ok(PolicyReceipt {
            location: subkey,
            outcome: PolicyWrite::SatisfiedByMachine,
            names: names.into_iter().map(str::to_owned).collect(),
        });
    }
    let outcome = if differing.is_empty() {
        PolicyWrite::AlreadyVerified(hive)
    } else {
        store.write_policy_values(hive, target, entries)?;
        let observed = store.read_policy_document(hive, target, &names)?;
        for (name, value) in entries {
            if observed.get(name) != Some(value) {
                return Err(mismatch(hive, target, name));
            }
        }
        PolicyWrite::Written(hive)
    };
    // Why: a machine key that appeared between the first check and the write
    // shadows the per-user value just written, so the write must not report
    // success.
    if hive == PolicyHive::User && store.policy_key_exists(PolicyHive::Machine, target)? {
        let machine = store.read_policy_document(PolicyHive::Machine, target, &names)?;
        let differing: Vec<String> = entries
            .iter()
            .filter(|(name, value)| machine.get(name) != Some(value))
            .map(|(name, _)| name.clone())
            .collect();
        if !differing.is_empty() {
            return Err(ConfigStoreError::HiveConflict { subkey, differing });
        }
    }
    Ok(PolicyReceipt {
        location: subkey,
        outcome,
        names: names.into_iter().map(str::to_owned).collect(),
    })
}

pub fn remove_values(
    store: &dyn ConfigStore,
    hive: PolicyHive,
    target: PolicyTarget,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    let removed = store.delete_policy_values(hive, target, names)?;
    let observed = store.read_policy_document(hive, target, names)?;
    if let Some(name) = names.iter().find(|name| observed.contains_key(**name)) {
        return Err(mismatch(hive, target, name));
    }
    Ok(removed)
}

fn mismatch(hive: PolicyHive, target: PolicyTarget, name: &str) -> ConfigStoreError {
    ConfigStoreError::VerifyMismatch {
        hive: hive.label().to_owned(),
        subkey: target.subkey(),
        name: name.to_owned(),
    }
}
