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
use crate::stdio::diag;
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
    bootstrap_install(&location, &binary, gateway_str)?;
    persist_optional_config(gateway_str, pubkey_str);

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    // Why: `inferenceGatewayBaseUrl` must stay loopback — the upstream gateway
    // URL must never be exposed to Cowork. The endpoint already names a proxy
    // that had to move off the default port, which this command cannot see
    // in-process because it runs separately from the proxy itself.
    let mdm = run_mdm_step(opts, target_os, loopback, &registry)?;

    let schedule = run_schedule_step(opts, &binary, bridge)?;

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
    bootstrap::write_version_sentinel(binary, gateway_url).map_err(InstallError::Sentinel)?;
    Ok(())
}

fn persist_optional_config(gateway_url: Option<&str>, pubkey: Option<&str>) {
    if let Some(url) = gateway_url
        && let Err(e) = config::ensure_gateway_url(url)
    {
        diag(&format!(
            "warning: could not persist gateway_url to config: {e}"
        ));
    }
    if let Some(pubkey) = pubkey {
        match config::persist_pinned_pubkey(pubkey) {
            Ok(()) => tracing::info!(
                pubkey_len = pubkey.len(),
                "pinned operator-supplied manifest pubkey"
            ),
            Err(e) => diag(&format!(
                "warning: failed to persist operator-supplied pubkey to local config: {e}"
            )),
        }
    }
}

fn run_mdm_step(
    opts: &InstallOptions,
    target_os: Os,
    loopback: &LoopbackEndpoint,
    registry: &McpRegistry,
) -> Result<MdmDisplay, InstallError> {
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    let inference_base_url = loopback.origin();
    let mcp = mdm::MdmPayloadInputs {
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
        .map(|lines| MdmDisplay::MobileconfigApplied { lines })
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
        .map(|lines| MdmDisplay::Applied {
            os: target_os,
            lines,
        })
        .map_err(InstallError::MdmApply)
}
