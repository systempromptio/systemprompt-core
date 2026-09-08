//! The Windows managed-policy write plan: what goes into Claude's hive, what
//! goes into the bridge's own key, which stale value to clear, and whether any
//! of it has drifted from the registry.
//!
//! The plan is addressed to one hive. An elevated process manages the machine
//! policy (`HKLM`); an ordinary one manages the per-user policy (`HKCU`),
//! which Claude honours as long as no machine policy exists. Drift is judged
//! against that hive alone — a value present in the *other* hive is not
//! evidence that this one is in step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use super::error::MdmError;
use crate::config::store::{self, PolicyHive, hive_for};

type Values = [(&'static str, &'static str, String)];

pub(super) struct WritePlan<'a> {
    claude: &'a Values,
    bridge: &'a Values,
    hive: PolicyHive,
    store: &'a store::PolicyStore,
}

impl<'a> WritePlan<'a> {
    pub(super) const fn new(
        claude: &'a Values,
        bridge: &'a Values,
        elevated: bool,
        store: &'a store::PolicyStore,
    ) -> Self {
        let hive = hive_for(elevated);
        Self {
            claude,
            bridge,
            hive,
            store,
        }
    }

    pub(super) const fn hive(&self) -> PolicyHive {
        self.hive
    }

    pub(super) fn write(&self) -> Result<Vec<store::verified::PolicyReceipt>, MdmError> {
        let claude = typed(self.claude);
        let bridge = typed(self.bridge);
        let mut completed = vec![
            store::verified::apply(
                self.store.backend(),
                self.hive,
                store::PolicyTarget::Claude,
                &claude,
            )
            .map_err(policy_err)?,
        ];
        if !bridge.is_empty() {
            let receipt = store::verified::apply(
                self.store.backend(),
                self.hive,
                store::PolicyTarget::Bridge,
                &bridge,
            )
            .map_err(|source| {
                policy_err(store::ConfigStoreError::Partial {
                    completed: completed.clone(),
                    source: Box::new(source),
                })
            })?;
            completed.push(receipt);
        }
        store::verified::remove_values(
            self.store.backend(),
            self.hive,
            store::PolicyTarget::Claude,
            &[store::LEGACY_MANIFEST_PUBKEY_KEY],
        )
        .map_err(|source| {
            policy_err(store::ConfigStoreError::Partial {
                completed: completed.clone(),
                source: Box::new(source),
            })
        })?;
        Ok(completed)
    }
}

fn typed(values: &Values) -> Vec<(String, store::PolicyDocumentValue)> {
    values
        .iter()
        .map(|(name, _, data)| {
            (
                (*name).to_owned(),
                store::PolicyDocumentValue::Str(data.clone()),
            )
        })
        .collect()
}

pub(super) const fn policy_err(e: store::ConfigStoreError) -> MdmError {
    MdmError::Store(e)
}
