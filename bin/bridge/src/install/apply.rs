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
use crate::ids::PinnedPubKey;
use crate::obs::output::diag;
use crate::schedule::Os;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::ValidatedUrl;

#[tracing::instrument(level = "info", skip(opts))]
pub fn install(opts: &InstallOptions) -> Result<InstallSummary, InstallError> {
    let binary = resolve_binary_path()?;
    let location = resolve_org_plugins()?;

    let gateway_str = opts.gateway_url.as_ref().map(ValidatedUrl::as_str);
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    bootstrap_install(&location, &binary, gateway_str)?;
    persist_optional_config(gateway_str, pubkey_str);

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    let inference_base_url = resolve_inference_base_url();
    let mdm = run_mdm_step(opts, target_os, &inference_base_url)?;

    let schedule = run_schedule_step(opts, &binary)?;

    Ok(InstallSummary {
        location,
        binary,
        mdm,
        schedule,
    })
}

// --apply-schedule wins over --emit-schedule-template: registering the job is
// a superset of printing instructions for registering it by hand.
fn run_schedule_step(
    opts: &InstallOptions,
    binary: &Path,
) -> Result<Option<ScheduleDisplay>, InstallError> {
    if opts.apply_schedule {
        return schedule_apply::apply_schedule(Os::current(), binary)
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

// MUST be loopback: the upstream gateway URL never appears in
// `inferenceGatewayBaseUrl`.
fn resolve_inference_base_url() -> String {
    if let Some(handle) = crate::proxy::handle() {
        return format!("http://127.0.0.1:{}", handle.port);
    }
    format!("http://127.0.0.1:{}", crate::proxy::DEFAULT_PROXY_PORT)
}

fn run_mdm_step(
    opts: &InstallOptions,
    target_os: Os,
    inference_base_url: &str,
) -> Result<MdmDisplay, InstallError> {
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    if opts.apply_mobileconfig {
        return run_apply_mobileconfig(inference_base_url, pubkey_str);
    }
    if opts.apply {
        return run_apply(target_os, inference_base_url, pubkey_str);
    }
    Ok(MdmDisplay::Snippet {
        os: target_os,
        snippet: mdm::snippet(target_os, Some(inference_base_url)),
    })
}

#[cfg(target_os = "macos")]
fn run_apply_mobileconfig(
    inference_base_url: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    mdm::macos::apply_mobileconfig(inference_base_url, pubkey)
        .map(|lines| MdmDisplay::MobileconfigApplied { lines })
        .map_err(InstallError::MobileconfigApply)
}

#[cfg(not(target_os = "macos"))]
const fn run_apply_mobileconfig(
    _inference_base_url: &str,
    _pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    Err(InstallError::MobileconfigUnsupported)
}

fn run_apply(
    target_os: Os,
    inference_base_url: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    mdm::apply_mdm(target_os, inference_base_url, pubkey)
        .map(|lines| MdmDisplay::Applied {
            os: target_os,
            lines,
        })
        .map_err(InstallError::MdmApply)
}
