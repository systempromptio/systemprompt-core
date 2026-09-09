//! Windows MDM (registry policy) deployment snippet rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use super::error::MdmError;
use super::windows_policy;
use crate::config::store::{PolicyHive, PolicyWrite};

pub(super) fn write_managed_mcp_servers_value(
    store: &crate::config::store::PolicyStore,
    value: &str,
) -> Result<String, MdmError> {
    let elevated = crate::winproc::is_elevated();
    let entries = [("managedMcpServers".to_owned(), value.to_owned())];
    let outcome = crate::config::store::verified::apply(
        store.backend(),
        crate::config::store::hive_for(elevated),
        crate::config::store::PolicyTarget::Claude,
        &crate::config::store::PolicyDocumentValue::strings(&entries),
    )
    .map_err(windows_policy::policy_err)?;
    if elevated {
        clear_stale_user_value("managedMcpServers")?;
    }
    Ok(match outcome.outcome() {
        PolicyWrite::AlreadyVerified(hive) => format!("{} already verified", policy_key(hive)),
        PolicyWrite::Written(hive) => format!("{} ← managedMcpServers", policy_key(hive)),
        PolicyWrite::SatisfiedByMachine => format!(
            "{} already holds this managedMcpServers value; the per-user copy was not written",
            crate::cowork_compat::HKLM_POLICY_KEY
        ),
    })
}

const fn policy_key(hive: PolicyHive) -> &'static str {
    match hive {
        PolicyHive::Machine => crate::cowork_compat::HKLM_POLICY_KEY,
        PolicyHive::User => crate::cowork_compat::HKCU_POLICY_KEY,
    }
}

fn clear_stale_user_value(name: &str) -> Result<(), MdmError> {
    crate::config::store::clear_managed_claude_policy(false, &[name])?;
    Ok(())
}

fn require_org_plugins() -> Result<(), MdmError> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()
        .map_err(|e| MdmError::Windows(e.to_string()))?;
    if org.path.is_dir() {
        return crate::windows_acl::verify_modify_tree(&org.path)
            .map_err(|e| MdmError::Windows(e.to_string()));
    }
    Err(MdmError::Windows(format!(
        "{} is not provisioned; run install --apply as Administrator before using Cowork",
        org.path.display()
    )))
}

pub(super) fn enforce_managed_policy(
    inputs: &super::MdmPayloadInputs<'_>,
) -> Result<String, MdmError> {
    ensure_workspace_dir()?;
    let values = policy_values(inputs, &inputs.loopback.origin())?;
    // Why: no `manifestTrust` is written here. HKLM policy is the administrator
    // channel: it outranks the operator's `gateway_url` and survives every
    // user-level reset, so a pin the bridge learned for itself became
    // unclearable and blocked pointing at a second gateway. Self-learned trust
    // is persisted per gateway in the config file; `install --apply --pubkey`
    // is the administrator pinning a key out of band and still writes it.
    let elevated = crate::winproc::is_elevated();
    let replaced = if elevated {
        foreign_secret_in_policy(inputs)
    } else {
        None
    };
    let plan = windows_policy::WritePlan::new(&values, &[], elevated, inputs.policy_store);

    let mut line = plan
        .write()?
        .iter()
        .map(crate::config::store::verified::PolicyReceipt::describe)
        .collect::<Vec<_>>()
        .join("; ");
    if let Some(fp) = replaced {
        line.push_str(&format!(
            "; replaced a machine policy carrying another bridge's secret (fingerprint {fp}) — \
             any other account's Claude Desktop on this computer now needs a repair"
        ));
    }
    if !elevated {
        require_org_plugins()?;
    }
    Ok(line)
}

// Why: the machine policy is shared by every account on the computer while
// the secret in it belongs to one bridge. An elevated sync that overwrites a
// secret it did not mint takes Claude Desktop away from whoever did, so the
// overwrite is recorded by fingerprint where doctor and the activity log show
// it rather than happening silently.
fn foreign_secret_in_policy(inputs: &super::MdmPayloadInputs<'_>) -> Option<String> {
    let ours = inputs.loopback.secret_fingerprint()?;
    let existing = inputs
        .policy_store
        .backend()
        .read_managed_policy(crate::cowork_compat::POLICY_API_KEY)
        .ok()
        .flatten()?;
    let theirs = crate::proxy::secret::fingerprint(existing.trim());
    if theirs == ours {
        return None;
    }
    tracing::warn!(
        target: "bridge::mdm",
        existing_fp = %theirs,
        ours_fp = %ours,
        "replacing a machine Claude policy written for another bridge secret"
    );
    Some(theirs)
}

// Why: Cowork pre-trusts allowedWorkspaceFolders only when the directory
// already exists.
fn ensure_workspace_dir() -> Result<Option<String>, MdmError> {
    let workspace = crate::brand::brand().workspace_dir_name;
    if workspace.is_empty() {
        return Ok(None);
    }
    let home = std::env::var_os("USERPROFILE").ok_or(MdmError::Resolve("USERPROFILE"))?;
    let ws = std::path::Path::new(&home).join(workspace);
    std::fs::create_dir_all(&ws).map_err(|source| MdmError::Io {
        action: "create workspace",
        path: ws.clone(),
        source,
    })?;
    Ok(Some(format!("ensured workspace dir {}", ws.display())))
}

pub(super) fn remove_policy() -> Result<bool, MdmError> {
    let store = crate::config::store::managed_policy_store();
    let hkcu = store
        .delete_policy_key(PolicyHive::User)
        .map_err(windows_policy::policy_err)?;
    let hklm = crate::config::store::verified::remove_values(
        store.as_ref(),
        PolicyHive::Machine,
        crate::config::store::PolicyTarget::Claude,
        &["managedMcpServers"],
    )? > 0;
    Ok(hkcu || hklm)
}

fn policy_values(
    inputs: &super::MdmPayloadInputs<'_>,
    base_url: &str,
) -> Result<Vec<(&'static str, &'static str, String)>, MdmError> {
    let secret = inputs.loopback.secret().map_err(|e| {
        MdmError::Windows(format!(
            "loopback secret unavailable ({e}); the gateway policy block was not written. Start \
             the Bridge proxy, then sync again."
        ))
    })?;
    let servers = super::policy::mcp_entries(inputs.loopback, inputs.registry).map_err(|e| {
        MdmError::Windows(format!("the MCP connector list could not be built: {e}"))
    })?;
    let existing_models = inputs
        .policy_store
        .backend()
        .read_managed_policy("inferenceModels")?;
    let policy = super::policy::claude_desktop_policy(&super::policy::PolicyInputs {
        base_url,
        api_key: secret.as_str(),
        models: existing_models,
        headers: &std::collections::BTreeMap::new(),
        egress_allowed_hosts: inputs.egress_allowed_hosts,
        org_uuid: crate::config::load()?
            .deployment_organization_uuid
            .as_deref(),
        mcp_servers: &servers,
    });
    Ok(super::policy::reg_values(&policy))
}

fn validate_gateway(gateway: &str) -> Result<(), MdmError> {
    let url = url::Url::parse(gateway).map_err(|e| MdmError::InvalidConfig(e.to_string()))?;
    let loopback = match url.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(MdmError::InsecureGateway {
            gateway: gateway.to_owned(),
        });
    }
    Ok(())
}

pub(super) fn apply(
    inputs: &super::MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<super::MdmApplication, MdmError> {
    validate_gateway(gateway)?;
    let elevated = crate::winproc::is_elevated();
    let values = policy_values(inputs, gateway)?;
    let bridge = super::bridge_policy_values(
        pubkey,
        &crate::config::gateway_url_or_default(&crate::config::load()?),
    )?;
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
        } else {
            require_org_plugins()?;
        }
        Ok::<_, MdmError>(())
    })();
    provisioning.map_err(|source| MdmError::Partial {
        completed: super::MdmApplication {
            lines: summary.clone(),
            policies: policies.clone(),
            files: Vec::new(),
        },
        source: Box::new(source),
    })?;
    summary.push("Fully quit Bridge (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(super::MdmApplication {
        lines: summary,
        policies,
        files: Vec::new(),
    })
}
