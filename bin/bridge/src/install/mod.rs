mod bootstrap;
mod builders;
mod error;
mod mdm;
mod schedule_emit;
mod summary;
#[cfg(target_os = "macos")]
pub(crate) mod xml;

pub use builders::{InstallOptionsBuilder, UninstallSummaryBuilder};
pub use error::InstallError;
pub use schedule_emit::emit_schedule;
pub use summary::{render_install_summary, render_uninstall_summary};

use crate::config::paths::{self, Scope};
use crate::config::{self as config};
use crate::ids::PinnedPubKey;
use crate::obs::output::diag;
use crate::schedule::Os;
#[cfg(target_os = "macos")]
pub use mdm::macos::{
    build_mobileconfig as build_macos_mobileconfig, build_prefs_plist as build_macos_prefs_plist,
};
pub use mdm::windows_policy_values;
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::ValidatedUrl;

pub struct InstallOptions {
    pub print_mdm: Option<Os>,
    pub emit_schedule_template: Option<Os>,
    pub gateway_url: Option<ValidatedUrl>,
    pub pubkey: Option<PinnedPubKey>,
    pub apply: bool,
    pub apply_mobileconfig: bool,
}

impl InstallOptions {
    #[must_use]
    pub fn builder() -> InstallOptionsBuilder {
        InstallOptionsBuilder::new()
    }
}

pub struct InstallSummary {
    pub location: paths::OrgPluginsLocation,
    pub binary: PathBuf,
    pub mdm: MdmDisplay,
    pub schedule: Option<ScheduleEmit>,
}

pub enum MdmDisplay {
    Snippet { os: Os, snippet: String },
    Applied { os: Os, lines: Vec<String> },
    MobileconfigApplied { lines: Vec<String> },
}

pub struct ScheduleEmit {
    pub os: Os,
    pub path: PathBuf,
    pub install_hint: String,
}

pub struct UninstallSummary {
    pub metadata_removed: Option<PathBuf>,
    pub metadata_already_clean: Option<PathBuf>,
    pub managed_profile: ManagedProfileOutcome,
    pub credentials: CredentialsOutcome,
}

impl UninstallSummary {
    #[must_use]
    pub fn builder() -> UninstallSummaryBuilder {
        UninstallSummaryBuilder::new()
    }
}

pub enum ManagedProfileOutcome {
    NotApplicable,
    Removed(&'static str),
    NotInstalled(&'static str),
    RemoveFailed(String),
}

pub enum CredentialsOutcome {
    Purged(PathBuf),
    Kept,
    PurgeFailed(String),
}

#[tracing::instrument(level = "info", skip(opts))]
pub fn install(opts: &InstallOptions) -> Result<InstallSummary, InstallError> {
    let binary = resolve_binary_path()?;
    let location = resolve_org_plugins()?;

    let gateway_str = opts.gateway_url.as_ref().map(ValidatedUrl::as_str);
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    bootstrap_install(&location, &binary, gateway_str)?;
    persist_optional_config(gateway_str, pubkey_str);

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    let gateway_for_mdm = resolve_gateway_for_mdm(gateway_str);
    let mdm = run_mdm_step(opts, target_os, &gateway_for_mdm)?;

    let schedule = match opts.emit_schedule_template {
        Some(os) => Some(schedule_emit::emit_schedule(os, &binary)?),
        None => None,
    };

    Ok(InstallSummary {
        location,
        binary,
        mdm,
        schedule,
    })
}

fn resolve_binary_path() -> Result<PathBuf, InstallError> {
    std::env::current_exe().map_err(InstallError::BinaryPath)
}

fn resolve_org_plugins() -> Result<paths::OrgPluginsLocation, InstallError> {
    paths::org_plugins_effective().ok_or(InstallError::OrgPluginsUnresolvable)
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
                    |_| "systemprompt-bridge".into(),
                    |p| p.display().to_string()
                ),
            )
        } else {
            format!("directory bootstrap failed: {e}")
        };
        return Err(InstallError::Bootstrap(msg));
    }
    bootstrap::write_version_sentinel(&location.path, binary, gateway_url)
        .map_err(InstallError::Sentinel)?;
    Ok(())
}

fn persist_optional_config(gateway_url: Option<&str>, pubkey: Option<&str>) {
    if let Some(url) = gateway_url
        && let Err(e) = config::ensure_gateway_url(url) {
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

fn resolve_gateway_for_mdm(cli_url: Option<&str>) -> String {
    cli_url
        .map(str::to_string)
        .or_else(|| config::load().gateway_url.map(|u| u.as_str().to_string()))
        .unwrap_or_else(|| "https://gateway.systemprompt.io".into())
}

fn run_mdm_step(
    opts: &InstallOptions,
    target_os: Os,
    gateway_for_mdm: &str,
) -> Result<MdmDisplay, InstallError> {
    let pubkey_str = opts.pubkey.as_ref().map(PinnedPubKey::as_str);
    if opts.apply_mobileconfig {
        return run_apply_mobileconfig(gateway_for_mdm, pubkey_str);
    }
    if opts.apply {
        return run_apply(target_os, gateway_for_mdm, pubkey_str);
    }
    Ok(MdmDisplay::Snippet {
        os: target_os,
        snippet: mdm::snippet(target_os, Some(gateway_for_mdm)),
    })
}

#[cfg(target_os = "macos")]
fn run_apply_mobileconfig(gateway: &str, pubkey: Option<&str>) -> Result<MdmDisplay, InstallError> {
    mdm::macos::apply_mobileconfig(gateway, pubkey)
        .map(|lines| MdmDisplay::MobileconfigApplied { lines })
        .map_err(InstallError::MobileconfigApply)
}

#[cfg(not(target_os = "macos"))]
fn run_apply_mobileconfig(
    _gateway: &str,
    _pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    Err(InstallError::MobileconfigUnsupported)
}

fn run_apply(
    target_os: Os,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, InstallError> {
    mdm::apply_mdm(target_os, gateway, pubkey)
        .map(|lines| MdmDisplay::Applied {
            os: target_os,
            lines,
        })
        .map_err(InstallError::MdmApply)
}

#[must_use]
pub fn os_label(os: Os) -> &'static str {
    mdm::os_label(os)
}

#[tracing::instrument(level = "info")]
pub fn uninstall(purge: bool) -> Result<UninstallSummary, InstallError> {
    let location = paths::org_plugins_effective().ok_or(InstallError::OrgPluginsUnresolvable)?;

    let metadata = paths::metadata_dir(&location.path);
    let (metadata_removed, metadata_already_clean) = if metadata.exists() {
        fs::remove_dir_all(&metadata).map_err(|e| {
            InstallError::Bootstrap(format!(
                "failed to remove metadata dir {}: {e}",
                metadata.display()
            ))
        })?;
        (Some(metadata.clone()), None)
    } else {
        (None, Some(metadata.clone()))
    };

    let staging = paths::staging_dir(&location.path);
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }

    let synthetic = location.path.join(paths::SYNTHETIC_PLUGIN_NAME);
    if synthetic.exists() {
        let _ = fs::remove_dir_all(&synthetic);
    }

    let managed_profile = remove_managed_profile();

    let credentials = if purge {
        match crate::auth::setup::logout() {
            Ok(p) => CredentialsOutcome::Purged(p.pat_file),
            Err(e) => {
                let msg = format!("credential purge failed: {e}");
                diag(&msg);
                CredentialsOutcome::PurgeFailed(msg)
            },
        }
    } else {
        CredentialsOutcome::Kept
    };

    Ok(UninstallSummary {
        metadata_removed,
        metadata_already_clean,
        managed_profile,
        credentials,
    })
}

#[cfg(target_os = "macos")]
fn remove_managed_profile() -> ManagedProfileOutcome {
    match mdm::macos::remove_profile() {
        Ok(true) => ManagedProfileOutcome::Removed(mdm::macos::PAYLOAD_IDENTIFIER),
        Ok(false) => ManagedProfileOutcome::NotInstalled(mdm::macos::PAYLOAD_IDENTIFIER),
        Err(e) => {
            let msg = format!("profile remove failed: {e}");
            diag(&msg);
            ManagedProfileOutcome::RemoveFailed(msg)
        },
    }
}

#[cfg(not(target_os = "macos"))]
fn remove_managed_profile() -> ManagedProfileOutcome {
    ManagedProfileOutcome::NotApplicable
}
