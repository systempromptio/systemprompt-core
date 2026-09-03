//! The Windows managed-policy write plan: what goes into Claude's hive, what
//! goes into the bridge's own key, which stale value to clear, and whether any
//! of it has drifted from the registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use super::error::MdmError;

type Values = [(&'static str, &'static str, String)];

pub(super) struct WritePlan<'a> {
    claude: &'a Values,
    bridge: &'a Values,
    clear_legacy: bool,
}

impl<'a> WritePlan<'a> {
    pub(super) fn new(claude: &'a Values, bridge: &'a Values) -> Self {
        // Why: the pin used to be written into Claude's hive; a leftover copy
        // there is what Claude Desktop warns about on every launch.
        let clear_legacy = matches!(
            crate::config::store::managed_policy_store()
                .read_managed_policy(super::LEGACY_PUBKEY_KEY),
            Ok(Some(_))
        );
        Self {
            claude,
            bridge,
            clear_legacy,
        }
    }

    // Why: a read error counts as drift — an unknown on-disk state must never
    // be mistaken for an up-to-date one.
    pub(super) fn drifted(&self) -> bool {
        let store = crate::config::store::managed_policy_store();
        self.clear_legacy
            || self.claude.iter().any(|(name, _, data)| {
                !matches!(store.read_managed_policy(name), Ok(Some(current)) if &current == data)
            })
            || self.bridge.iter().any(|(name, _, data)| {
                crate::config::store::read_bridge_policy(name).as_ref() != Some(data)
            })
    }

    pub(super) fn write_in_process(&self) -> Result<(), MdmError> {
        reg_add_all(crate::cowork_compat::HKLM_POLICY_KEY, self.claude)?;
        let bridge_key = format!(r"HKLM\{}", crate::config::store::bridge_policy_subkey());
        reg_add_all(&bridge_key, self.bridge)?;
        if self.clear_legacy {
            _ = crate::winproc::reg_command()
                .args([
                    "delete",
                    crate::cowork_compat::HKLM_POLICY_KEY,
                    "/v",
                    super::LEGACY_PUBKEY_KEY,
                    "/f",
                ])
                .status();
        }
        Ok(())
    }

    // Why: the error shown when Cowork sync cannot write org-plugins tells the
    // user to re-run `install --apply` and approve ONE administrator prompt —
    // this is the step that makes that promise true: one UAC pass writes both
    // policy keys and grants the invoking user Modify on org-plugins.
    pub(super) fn stage_elevated(
        &self,
        org_plugins: Option<crate::install::elevated_job::OrgPluginsJob>,
    ) -> Result<String, MdmError> {
        let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
        std::fs::create_dir_all(&dir).map_err(|source| MdmError::Io {
            action: "create staging dir",
            path: dir.clone(),
            source,
        })?;
        let entries: Vec<(&str, String)> = self
            .claude
            .iter()
            .map(|(n, _, d)| (*n, d.clone()))
            .collect();
        let body = crate::install::reg_values::render_reg_values(true, &entries);
        let path = dir.join("bridge-policy-apply.reg");
        std::fs::write(&path, body).map_err(|source| MdmError::Io {
            action: "stage policy profile",
            path: path.clone(),
            source,
        })?;
        let job = crate::install::elevated_job::ElevatedJob {
            clear_values: if self.clear_legacy {
                vec![super::LEGACY_PUBKEY_KEY.to_owned()]
            } else {
                Vec::new()
            },
            bridge_values: self
                .bridge
                .iter()
                .map(|(n, _, d)| ((*n).to_owned(), d.clone()))
                .collect(),
            managed_files: Vec::new(),
            remove_files: Vec::new(),
            reg_path: Some(path.to_string_lossy().into_owned()),
            org_plugins,
        };
        crate::install::elevated_job::elevate_and_run(&dir, &job)
            .map(|()| {
                "elevated step complete: HKLM policy written and org-plugins provisioned".to_owned()
            })
            .map_err(|e| {
                MdmError::Windows(format!(
                    "elevated step did not complete ({e}); the machine policy was not written and \
                     org-plugins was not provisioned — Cowork stays unmanaged"
                ))
            })
    }
}

fn reg_add_all(key: &str, values: &Values) -> Result<(), MdmError> {
    for (name, kind, data) in values {
        let status = crate::winproc::reg_command()
            .args(["add", key, "/v", name, "/t", kind, "/d", data, "/f"])
            .status()
            .map_err(|e| MdmError::Windows(format!("reg add {name}: {e}")))?;
        if !status.success() {
            return Err(MdmError::Windows(format!(
                "reg add {name} exited with {}",
                status.code().unwrap_or(-1)
            )));
        }
    }
    Ok(())
}
