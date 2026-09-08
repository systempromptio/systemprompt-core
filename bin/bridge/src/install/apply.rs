//! Install orchestration: directory bootstrap, optional config persistence,
//! MDM step dispatch, and schedule template emission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    InstallError, InstallOptions, InstallSummary, MdmDisplay, ScheduleDisplay, bootstrap, mdm,
    schedule_apply, schedule_emit,
};
use crate::config::paths::{self, Scope};
use crate::config::{self as config};
use crate::context::BridgeContext;
use crate::ids::PinnedPubKey;
use crate::mcp_registry::McpRegistry;
use crate::proxy::LoopbackEndpoint;
use crate::schedule::Os;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::ValidatedUrl;

#[tracing::instrument(level = "info", skip(opts))]
pub fn install(
    opts: &InstallOptions,
    bridge: &BridgeContext,
) -> Result<InstallSummary, InstallError> {
    let loopback = bridge.proxy.loopback();
    let registry = bridge.mcp_registry();
    let binary = resolve_binary_path()?;
    let location = resolve_org_plugins()?;

    let gateway_str = opts.gateway_url.as_ref().map(ValidatedUrl::as_str);
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    let mut completed = Vec::new();
    let outcome = (|| {
        bootstrap_install(&location, &binary, gateway_str, &mut completed)?;
        persist_optional_config(gateway_str, pubkey_str, &mut completed)?;
        let target_os = opts.print_mdm.unwrap_or_else(Os::current);
        let mdm = run_mdm_step(opts, target_os, loopback, &registry, &bridge.policy_store)?;
        completed.push(super::InstallStep::Policy {
            outcome: mdm.clone(),
        });
        let schedule = run_schedule_step(opts, &binary, bridge)?;
        if let Some(schedule) = &schedule {
            completed.push(super::InstallStep::Schedule {
                outcome: schedule.clone(),
            });
        }
        Ok::<_, InstallError>((mdm, schedule))
    })();
    let (mdm, schedule) = outcome.map_err(|source| InstallError::Partial {
        completed: completed.clone(),
        source: Box::new(source),
    })?;

    Ok(InstallSummary {
        location,
        binary,
        mdm,
        schedule,
    })
}

fn run_schedule_step(
    opts: &InstallOptions,
    binary: &Path,
    bridge: &BridgeContext,
) -> Result<Option<ScheduleDisplay>, InstallError> {
    if opts.apply_schedule {
        return schedule_apply::apply_schedule(&bridge.schedule, Os::current(), binary)
            .map(|a| Some(ScheduleDisplay::Applied(a)));
    }
    opts.emit_schedule_template.map_or(Ok(None), |os| {
        schedule_emit::emit_schedule(os, binary).map(|e| Some(ScheduleDisplay::Template(e)))
    })
}

fn resolve_binary_path() -> Result<PathBuf, InstallError> {
    std::env::current_exe().map_err(InstallError::BinaryPath)
}

fn resolve_org_plugins() -> Result<paths::OrgPluginsLocation, InstallError> {
    paths::org_plugins_install_target().ok_or(InstallError::OrgPluginsUnresolvable)
}

fn bootstrap_install(
    location: &paths::OrgPluginsLocation,
    binary: &Path,
    gateway_url: Option<&str>,
    completed: &mut Vec<super::InstallStep>,
) -> Result<(), InstallError> {
    if let Err(e) = bootstrap::bootstrap_directory(location) {
        let msg = if e.kind() == std::io::ErrorKind::PermissionDenied
            && matches!(location.scope, Scope::System)
        {
            format!(
                "permission denied creating {} — Claude Desktop only reads org plugins from the \
                 system path. Re-run as root: `sudo {} install --apply` (or use the install \
                 script). Underlying error: {e}",
                location.path.display(),
                std::env::current_exe().map_or_else(
                    |_| crate::brand::brand().binary_name.to_owned(),
                    |p| p.display().to_string()
                ),
            )
        } else {
            format!("directory bootstrap failed: {e}")
        };
        return Err(InstallError::Bootstrap(msg));
    }
    completed.push(super::InstallStep::Directory(location.path.clone()));
    bootstrap::write_version_sentinel(binary, gateway_url).map_err(InstallError::Sentinel)?;
    completed.push(super::InstallStep::Sentinel(
        paths::bridge_metadata_dir()
            .ok_or(InstallError::OrgPluginsUnresolvable)?
            .join(paths::VERSION_SENTINEL),
    ));
    Ok(())
}

fn persist_optional_config(
    gateway_url: Option<&str>,
    pubkey: Option<&str>,
    completed: &mut Vec<super::InstallStep>,
) -> Result<(), InstallError> {
    if let Some(url) = gateway_url {
        config::write::edit(|doc| config::write::set(doc, &["gateway_url"], url))?;
        completed.push(super::InstallStep::GatewayConfigured);
    }
    if let Some(pubkey) = pubkey {
        let cfg = config::load()?;
        config::persist_pinned_pubkey(&config::gateway_url_or_default(&cfg), pubkey)?;
        completed.push(super::InstallStep::TrustConfigured);
    }
    Ok(())
}

fn run_mdm_step(
    opts: &InstallOptions,
    target_os: Os,
    loopback: &LoopbackEndpoint,
    registry: &McpRegistry,
    policy_store: &config::store::PolicyStore,
) -> Result<MdmDisplay, InstallError> {
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    let inference_base_url = loopback.origin();
    let mcp = mdm::MdmPayloadInputs {
        policy_store,
        loopback,
        registry,
        egress_allowed_hosts: opts.egress_allowed_hosts.as_deref(),
    };
    if opts.apply_mobileconfig {
        return run_apply_mobileconfig(&mcp, &inference_base_url, pubkey_str);
    }
    if opts.apply {
        return run_apply(target_os, &mcp, &inference_base_url, pubkey_str);
    }
    Ok(MdmDisplay::Snippet {
        os: target_os,
        snippet: mdm::snippet(target_os, Some(&inference_base_url)),
    })
}

#[cfg(target_os = "macos")]
fn run_apply_mobileconfig(
    mcp: &mdm::MdmPayloadInputs<'_>,
    inference_base_url: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    mdm::macos::apply_mobileconfig(mcp, inference_base_url, pubkey)
        .map(|lines| MdmDisplay::MobileconfigPrepared { lines })
        .map_err(InstallError::MobileconfigApply)
}

#[cfg(not(target_os = "macos"))]
const fn run_apply_mobileconfig(
    _mcp: &mdm::MdmPayloadInputs<'_>,
    _inference_base_url: &str,
    _pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    Err(InstallError::MobileconfigUnsupported)
}

fn run_apply(
    target_os: Os,
    mcp: &mdm::MdmPayloadInputs<'_>,
    inference_base_url: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    mdm::apply_mdm(target_os, mcp, inference_base_url, pubkey)
        .map(|report| MdmDisplay::Applied {
            os: target_os,
            report,
        })
        .map_err(InstallError::MdmApply)
}
