mod bootstrap;
mod mdm;
#[cfg(target_os = "macos")]
pub(crate) mod xml;

#[cfg(target_os = "macos")]
pub use mdm::macos::{
    build_mobileconfig as build_macos_mobileconfig, build_prefs_plist as build_macos_prefs_plist,
};
pub use mdm::windows_policy_values;

use crate::config;
use crate::output::diag;
use crate::paths::{self, Scope};
use crate::schedule::{self, Os};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub struct InstallOptions {
    pub print_mdm: Option<Os>,
    pub emit_schedule_template: Option<Os>,
    pub gateway_url: Option<String>,
    pub pubkey: Option<String>,
    pub apply: bool,
    pub apply_mobileconfig: bool,
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

pub fn install(opts: InstallOptions) -> Result<InstallSummary, ExitCode> {
    let binary = resolve_binary_path()?;
    let location = resolve_org_plugins()?;

    bootstrap_install(&location, &binary, opts.gateway_url.as_deref())?;
    persist_optional_config(opts.gateway_url.as_deref(), opts.pubkey.as_deref());

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    let gateway_for_mdm = resolve_gateway_for_mdm(opts.gateway_url.as_deref());
    let mdm = run_mdm_step(&opts, target_os, &binary, &gateway_for_mdm)?;

    let schedule = match opts.emit_schedule_template {
        Some(os) => Some(emit_schedule(os, &binary)?),
        None => None,
    };

    Ok(InstallSummary {
        location,
        binary,
        mdm,
        schedule,
    })
}

fn resolve_binary_path() -> Result<PathBuf, ExitCode> {
    std::env::current_exe().map_err(|e| {
        diag(&format!("cannot determine current executable path: {e}"));
        ExitCode::from(1)
    })
}

fn resolve_org_plugins() -> Result<paths::OrgPluginsLocation, ExitCode> {
    paths::org_plugins_effective().ok_or_else(|| {
        diag("cannot resolve org-plugins directory for this OS");
        ExitCode::from(1)
    })
}

fn bootstrap_install(
    location: &paths::OrgPluginsLocation,
    binary: &Path,
    gateway_url: Option<&str>,
) -> Result<(), ExitCode> {
    if let Err(e) = bootstrap::bootstrap_directory(location) {
        if e.kind() == std::io::ErrorKind::PermissionDenied
            && matches!(location.scope, Scope::System)
        {
            diag(&format!(
                "permission denied creating {} — Claude Desktop only reads org plugins from the \
                 system path. Re-run as root: `sudo {} install --apply` (or use the install \
                 script). Underlying error: {e}",
                location.path.display(),
                std::env::current_exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "systemprompt-cowork".into()),
            ));
        } else {
            diag(&format!("directory bootstrap failed: {e}"));
        }
        return Err(ExitCode::from(1));
    }
    if let Err(e) = bootstrap::write_version_sentinel(&location.path, binary, gateway_url) {
        diag(&format!("version sentinel write failed: {e}"));
        return Err(ExitCode::from(1));
    }
    Ok(())
}

fn persist_optional_config(gateway_url: Option<&str>, pubkey: Option<&str>) {
    if let Some(url) = gateway_url {
        if let Err(e) = config::ensure_gateway_url(url) {
            diag(&format!(
                "warning: could not persist gateway_url to config: {e}"
            ));
        }
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
        .or_else(|| config::load().gateway_url)
        .unwrap_or_else(|| "https://gateway.systemprompt.io".into())
}

fn run_mdm_step(
    opts: &InstallOptions,
    target_os: Os,
    binary: &Path,
    gateway_for_mdm: &str,
) -> Result<MdmDisplay, ExitCode> {
    if opts.apply_mobileconfig {
        return run_apply_mobileconfig(binary, gateway_for_mdm, opts.pubkey.as_deref());
    }
    if opts.apply {
        return run_apply(target_os, binary, gateway_for_mdm, opts.pubkey.as_deref());
    }
    Ok(MdmDisplay::Snippet {
        os: target_os,
        snippet: mdm::snippet(target_os, binary, Some(gateway_for_mdm)),
    })
}

#[cfg(target_os = "macos")]
fn run_apply_mobileconfig(
    binary: &Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, ExitCode> {
    match mdm::macos::apply_mobileconfig(binary, gateway, pubkey) {
        Ok(lines) => Ok(MdmDisplay::MobileconfigApplied { lines }),
        Err(e) => {
            diag(&format!("apply --mobileconfig failed: {e}"));
            Err(ExitCode::from(1))
        },
    }
}

#[cfg(not(target_os = "macos"))]
fn run_apply_mobileconfig(
    _binary: &Path,
    _gateway: &str,
    _pubkey: Option<&str>,
) -> Result<MdmDisplay, ExitCode> {
    diag("--apply-mobileconfig is only supported on macOS");
    Err(ExitCode::from(1))
}

fn run_apply(
    target_os: Os,
    binary: &Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<MdmDisplay, ExitCode> {
    match mdm::apply_mdm(target_os, binary, gateway, pubkey) {
        Ok(lines) => Ok(MdmDisplay::Applied {
            os: target_os,
            lines,
        }),
        Err(e) => {
            diag(&format!("apply failed: {e}"));
            Err(ExitCode::from(1))
        },
    }
}

fn emit_schedule(schedule_os: Os, binary: &Path) -> Result<ScheduleEmit, ExitCode> {
    let filename = schedule::template_filename(schedule_os);
    let content = schedule::template(schedule_os, binary);
    let out = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(filename);
    if let Err(e) = fs::write(&out, content) {
        diag(&format!("failed to write {}: {e}", out.display()));
        return Err(ExitCode::from(1));
    }
    Ok(ScheduleEmit {
        os: schedule_os,
        path: out,
        install_hint: schedule::install_hint(schedule_os).to_string(),
    })
}

pub fn os_label(os: Os) -> &'static str {
    mdm::os_label(os)
}

pub fn uninstall(purge: bool) -> Result<UninstallSummary, ExitCode> {
    let location = paths::org_plugins_effective().ok_or_else(|| {
        diag("cannot resolve org-plugins directory for this OS");
        ExitCode::from(1)
    })?;

    let metadata = paths::metadata_dir(&location.path);
    let (metadata_removed, metadata_already_clean) = if metadata.exists() {
        if let Err(e) = fs::remove_dir_all(&metadata) {
            diag(&format!(
                "failed to remove metadata dir {}: {e}",
                metadata.display()
            ));
            return Err(ExitCode::from(1));
        }
        (Some(metadata.clone()), None)
    } else {
        (None, Some(metadata.clone()))
    };

    let staging = paths::staging_dir(&location.path);
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }

    let managed_profile = remove_managed_profile();

    let credentials = if purge {
        match crate::setup::logout() {
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

pub fn render_install_summary(s: &InstallSummary) -> String {
    let mut out = String::new();
    out.push_str("Installed systemprompt-cowork integration\n");
    out.push_str(&format!(
        "  org-plugins: {} ({})\n",
        s.location.path.display(),
        match s.location.scope {
            Scope::System => "system-wide",
            Scope::User => "per-user",
        }
    ));
    let meta = paths::metadata_dir(&s.location.path);
    out.push_str(&format!("  metadata:    {}\n", meta.display()));
    out.push_str(&format!(
        "    user.json:    {}\n",
        meta.join(paths::USER_FRAGMENT).display()
    ));
    out.push_str(&format!(
        "    skills/:      {}\n",
        meta.join(paths::SKILLS_DIR).display()
    ));
    out.push_str(&format!(
        "    agents/:      {}\n",
        meta.join(paths::AGENTS_DIR).display()
    ));
    out.push_str(&format!(
        "    managed-mcp:  {}\n",
        meta.join(paths::MANAGED_MCP_FRAGMENT).display()
    ));
    out.push_str(&format!("  binary:      {}\n", s.binary.display()));
    out.push_str(
        "  Run `systemprompt-cowork sync` to populate user identity, skills, agents, and MCP \
         servers.\n",
    );

    match &s.mdm {
        MdmDisplay::Snippet { os, snippet } => {
            out.push('\n');
            out.push_str(&format!("--- MDM configuration ({}) ---\n", os_label(*os)));
            out.push_str(snippet);
            if !snippet.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("Tip: rerun with --apply to write these keys directly.\n");
        },
        MdmDisplay::Applied { os, lines } => {
            out.push('\n');
            out.push_str(&format!("--- policy applied ({}) ---\n", os_label(*os)));
            for line in lines {
                out.push_str(&format!("  {line}\n"));
            }
        },
        MdmDisplay::MobileconfigApplied { lines } => {
            out.push('\n');
            out.push_str("--- mobileconfig applied (macOS) ---\n");
            for line in lines {
                out.push_str(&format!("  {line}\n"));
            }
        },
    }

    if let Some(sched) = &s.schedule {
        out.push('\n');
        out.push_str(&format!(
            "--- Schedule template ({}) ---\n",
            os_label(sched.os)
        ));
        out.push_str(&format!("wrote: {}\n", sched.path.display()));
        out.push_str(&sched.install_hint);
        if !sched.install_hint.ends_with('\n') {
            out.push('\n');
        }
    }

    out
}

pub fn render_uninstall_summary(s: &UninstallSummary) -> String {
    let mut out = String::new();
    if let Some(p) = &s.metadata_removed {
        out.push_str(&format!("Removed {}\n", p.display()));
    }
    if let Some(p) = &s.metadata_already_clean {
        out.push_str(&format!("No metadata dir at {} (already clean)\n", p.display()));
    }
    match &s.managed_profile {
        ManagedProfileOutcome::Removed(id) => {
            out.push_str(&format!("Removed managed profile {id}\n"));
        },
        ManagedProfileOutcome::NotInstalled(id) => {
            out.push_str(&format!(
                "No managed profile {id} installed (nothing to remove)\n"
            ));
        },
        ManagedProfileOutcome::RemoveFailed(_) | ManagedProfileOutcome::NotApplicable => {},
    }
    match &s.credentials {
        CredentialsOutcome::Purged(p) => {
            out.push_str(&format!("Purged credentials: {}\n", p.display()));
        },
        CredentialsOutcome::Kept => {
            out.push_str(
                "Credentials left intact. Use `systemprompt-cowork uninstall --purge` to also \
                 clear them.\n",
            );
        },
        CredentialsOutcome::PurgeFailed(_) => {},
    }
    out
}
