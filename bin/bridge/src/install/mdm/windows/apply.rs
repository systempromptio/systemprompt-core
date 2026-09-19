//! Windows policy installation and machine provisioning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ensure_workspace_dir, policy_key, policy_values, require_org_plugins, validate_gateway,
};
use crate::install::mdm::error::MdmError;
use crate::install::mdm::{self, windows_policy};

pub(in crate::install::mdm) fn apply(
    inputs: &mdm::MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<mdm::MdmApplication, MdmError> {
    validate_gateway(gateway)?;
    let elevated = crate::winproc::is_elevated();
    let values = policy_values(inputs, gateway)?;
    let cfg = crate::config::load()?;
    let gateway_url = crate::config::gateway_url_or_default(&cfg);
    // Why: the machine anchor is what the policy writer verifies manifests
    // against, and an elevated install is the one moment it can be written.
    // A pin the operator already holds is carried up rather than asking for
    // --pubkey again; no pin at all leaves the anchor unwritten and the
    // writer unregistered.
    let pinned = match pubkey {
        Some(key) => Some(key.to_owned()),
        None if elevated => {
            match crate::config::trust::pinned_pubkey_state_for(&cfg, &gateway_url)? {
                crate::config::PinnedPubkeyState::Pinned { key, .. } => {
                    Some(key.as_str().to_owned())
                },
                crate::config::PinnedPubkeyState::Unpinned
                | crate::config::PinnedPubkeyState::StaleForGateway { .. } => None,
            }
        },
        None => None,
    };
    let bridge = mdm::bridge_policy_values(pinned.as_deref(), &gateway_url)?;
    let plan = windows_policy::WritePlan::new(&values, &bridge, elevated, inputs.policy_store);
    let key = policy_key(plan.hive());
    let mut summary = Vec::with_capacity(values.len() + bridge.len() + 4);
    summary.push(format!("registry key: {key}"));
    summary.extend(ensure_workspace_dir()?);
    let policies = plan.write()?;
    summary.extend(
        policies
            .iter()
            .map(crate::config::store::verified::PolicyReceipt::describe),
    );
    let provisioning = (|| {
        if elevated {
            {
                let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()
                    .map_err(|e| MdmError::Windows(e.to_string()))?;
                crate::install::elevated_job::provision_org_plugins(&org.path, &org.grant_user)
                    .map_err(|e| {
                        MdmError::Windows(format!("org-plugins provisioning failed: {e}"))
                    })?;
                crate::windows_acl::verify_modify_tree(&org.path)
                    .map_err(|e| MdmError::Windows(e.to_string()))?;
                summary.push(format!(
                    "provisioned {} with a Modify grant for {}",
                    org.path.display(),
                    org.grant_user
                ));
            }
            // Why: registering the writer is what makes every later sync
            // prompt-free; without a machine trust anchor it cannot verify a
            // manifest, so it is not registered and the install says so.
            match std::env::current_exe()
                .map_err(|e| crate::install::policy_writer::PolicyWriterError::Io {
                    context: "locate this binary".to_owned(),
                    source: e,
                })
                .and_then(|exe| crate::install::policy_writer::install(&exe))
            {
                Ok(lines) => summary.extend(lines),
                Err(crate::install::policy_writer::PolicyWriterError::NoAnchor) => {
                    summary.push(
                        "policy writer not registered: no signing trust anchor in the machine \
                         policy (pass --pubkey, or pin the gateway key first); later connector \
                         changes will ask for administrator approval"
                            .to_owned(),
                    );
                },
                Err(e) => return Err(MdmError::Windows(format!("policy writer: {e}"))),
            }
        } else {
            require_org_plugins()?;
        }
        Ok::<_, MdmError>(())
    })();
    provisioning.map_err(|source| MdmError::Partial {
        completed: mdm::MdmApplication {
            lines: summary.clone(),
            policies: policies.clone(),
            files: Vec::new(),
        },
        source: Box::new(source),
    })?;
    summary.push("Fully quit Bridge (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(mdm::MdmApplication {
        lines: summary,
        policies,
        files: Vec::new(),
    })
}
