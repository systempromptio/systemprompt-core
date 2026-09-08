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
use crate::config::store::{self, PolicyHive, PolicyWrite, hive_for, managed_policy_store};

type Values = [(&'static str, &'static str, String)];

pub(super) struct WritePlan<'a> {
    claude: &'a Values,
    bridge: &'a Values,
    hive: PolicyHive,
    clear_legacy: bool,
}

impl<'a> WritePlan<'a> {
    pub(super) fn new(claude: &'a Values, bridge: &'a Values, elevated: bool) -> Self {
        let hive = hive_for(elevated);
        let clear_legacy = matches!(
            managed_policy_store().read_policy_document(hive, &[super::LEGACY_PUBKEY_KEY]),
            Ok(doc) if !doc.is_empty()
        );
        Self {
            claude,
            bridge,
            hive,
            clear_legacy,
        }
    }

    pub(super) const fn hive(&self) -> PolicyHive {
        self.hive
    }

    pub(super) fn drifted(&self) -> bool {
        if self.clear_legacy {
            return true;
        }
        let names: Vec<&str> = self.claude.iter().map(|(n, _, _)| *n).collect();
        let current = match managed_policy_store().read_policy_document(self.hive, &names) {
            Ok(doc) => doc,
            Err(e) => {
                tracing::warn!(
                    target: "bridge::install::mdm",
                    hive = self.hive.label(),
                    error = %e,
                    "could not read the current Claude policy; treating it as drifted"
                );
                return true;
            },
        };
        self.claude
            .iter()
            .any(|(name, _, data)| current.get(*name).and_then(|v| v.as_str()) != Some(data.as_str()))
            || self.bridge.iter().any(|(name, _, data)| {
                !matches!(store::read_bridge_policy_in(self.hive, name), Ok(Some(v)) if &v == data)
            })
    }

    /// Write the plan into its hive and read every value back. Returns what
    /// happened to the Claude block, which is `SatisfiedByMachine` when a
    /// per-user plan found identical values already in `HKLM`.
    pub(super) fn write(&self) -> Result<PolicyWrite, MdmError> {
        let claude: Vec<(String, String)> = self
            .claude
            .iter()
            .map(|(n, _, d)| ((*n).to_owned(), d.clone()))
            .collect();
        let elevated = self.hive == PolicyHive::Machine;
        let outcome = store::write_managed_claude_policy(elevated, &claude).map_err(policy_err)?;
        let bridge: Vec<(String, String)> = self
            .bridge
            .iter()
            .map(|(n, _, d)| ((*n).to_owned(), d.clone()))
            .collect();
        if !bridge.is_empty() {
            store::write_bridge_policy(elevated, &bridge).map_err(policy_err)?;
        }
        if self.clear_legacy {
            match store::clear_managed_claude_policy(elevated, &[super::LEGACY_PUBKEY_KEY]) {
                Ok(n) => tracing::info!(
                    target: "bridge::install::mdm",
                    hive = self.hive.label(),
                    removed = n,
                    "cleared legacy manifest pubkey value"
                ),
                Err(e) => tracing::warn!(
                    target: "bridge::install::mdm",
                    hive = self.hive.label(),
                    error = %e,
                    "legacy manifest pubkey value could not be cleared"
                ),
            }
        }
        Ok(outcome)
    }
}

pub(super) fn policy_err(e: store::ConfigStoreError) -> MdmError {
    MdmError::Windows(e.to_string())
}
