//! Windows MDM (registry policy) deployment snippet rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

mod apply;

pub(super) use self::apply::apply;

use super::error::MdmError;
use super::windows_policy;
use crate::config::store::PolicyHive;

const fn policy_key(hive: PolicyHive) -> &'static str {
    match hive {
        PolicyHive::Machine => crate::cowork_compat::HKLM_POLICY_KEY,
        PolicyHive::User => crate::cowork_compat::HKCU_POLICY_KEY,
    }
}

fn require_org_plugins() -> Result<(), MdmError> {
    let org =
        crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user().map_err(|e| {
            MdmError::OrgPlugins {
                context: "location",
                source: e.into(),
            }
        })?;
    if org.path.is_dir() {
        return crate::windows_acl::verify_modify_tree(&org.path).map_err(|e| {
            MdmError::OrgPlugins {
                context: "access check",
                source: e.into(),
            }
        });
    }
    Err(MdmError::OrgPluginsMissing { path: org.path })
}

// Why: the writer is tried before the per-user write so an unelevated sync
// reaches the machine hive without a prompt; `Ok(None)` means the writer is
// not on this computer and the caller takes the ordinary path, an `Err` is
// a writer that exists and failed, which the caller reports and then falls
// back on — a failed delegation is never a silent success.
pub(super) fn delegate_to_writer(
    inputs: &super::MdmPayloadInputs<'_>,
    facts: crate::install::policy_writer::RequestFacts,
) -> Result<Option<String>, crate::install::policy_writer::PolicyWriterError> {
    use crate::install::policy_writer::{self, PolicyWriterError, WriterStatus};
    if crate::winproc::is_elevated() {
        return Ok(None);
    }
    match policy_writer::status() {
        WriterStatus::Ready => {},
        WriterStatus::NotRegistered => return Ok(None),
        WriterStatus::Unavailable(why) => return Err(PolicyWriterError::Unavailable(why)),
    }
    let Some(fragment) =
        crate::mcp_registry::read_envelope().map_err(|source| PolicyWriterError::Io {
            context: "read the last verified manifest envelope".to_owned(),
            source,
        })?
    else {
        return Err(PolicyWriterError::Unavailable(
            "no verified manifest envelope has been kept yet; sync once first".to_owned(),
        ));
    };
    let catalog = super::tool_catalog::read().map_err(|source| PolicyWriterError::Io {
        context: "read the tool catalog".to_owned(),
        source,
    })?;
    let requester = crate::windows_acl::current_sid().map_err(|source| PolicyWriterError::Io {
        context: "resolve the requesting account".to_owned(),
        source,
    })?;
    let loopback =
        policy_writer::Loopback::of(inputs.loopback).map_err(|source| PolicyWriterError::Io {
            context: "read the loopback secret".to_owned(),
            source,
        })?;
    let request = policy_writer::build_request(loopback, &fragment, catalog, facts, requester);
    let steps = policy_writer::write_policy(&request)?;
    let receipts: Vec<String> = steps
        .iter()
        .flat_map(|step| step.policies.iter())
        .map(crate::config::store::verified::PolicyReceipt::describe)
        .collect();
    Ok(Some(format!(
        "{} ← full policy via the elevated writer ({})",
        crate::cowork_compat::HKLM_POLICY_KEY,
        receipts.join("; ")
    )))
}

pub(super) fn enforce_managed_policy(
    inputs: &super::MdmPayloadInputs<'_>,
) -> Result<String, MdmError> {
    ensure_workspace_dir()?;
    let values = policy_values(inputs)?;
    // Why: HKLM policy is the administrator channel and outranks the config
    // file, so self-learned trust is never written here — only
    // `install --apply --pubkey` pins a key out of band.
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

// Why: only the values the bridge wrote are removed; the policy key itself
// and any value another administrator placed there stay.
pub(super) fn remove_policy(store: &crate::config::store::PolicyStore) -> Result<bool, MdmError> {
    let hkcu = crate::config::store::verified::remove_values(
        store.backend(),
        PolicyHive::User,
        crate::config::store::PolicyTarget::Claude,
        super::policy::WRITTEN_POLICY_KEYS,
    )? > 0;
    let hklm = crate::config::store::verified::remove_values(
        store.backend(),
        PolicyHive::Machine,
        crate::config::store::PolicyTarget::Claude,
        &["managedMcpServers"],
    )? > 0;
    Ok(hkcu || hklm)
}

fn policy_values(
    inputs: &super::MdmPayloadInputs<'_>,
) -> Result<Vec<(&'static str, &'static str, String)>, MdmError> {
    Ok(super::policy::reg_values(&super::desktop_policy_entries(
        inputs,
    )?))
}

fn validate_gateway(gateway: &str) -> Result<(), MdmError> {
    let url = url::Url::parse(gateway)?;
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
