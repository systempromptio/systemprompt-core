mod bootstrap;
mod macos;
mod mdm;
#[cfg(target_os = "macos")]
pub(crate) mod xml;

#[cfg(target_os = "macos")]
pub use macos::{
    build_mobileconfig as build_macos_mobileconfig, build_prefs_plist as build_macos_prefs_plist,
};
pub use mdm::windows_policy_values;

use crate::config;
use crate::output::diag;
use crate::paths::{self, Scope};
use crate::schedule::{self, Os};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

pub struct InstallOptions {
    pub print_mdm: Option<Os>,
    pub emit_schedule_template: Option<Os>,
    pub gateway_url: Option<String>,
    pub pubkey: Option<String>,
    pub apply: bool,
    pub apply_mobileconfig: bool,
}

pub fn install(opts: InstallOptions) -> ExitCode {
    let binary = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            diag(&format!("cannot determine current executable path: {e}"));
            return ExitCode::from(1);
        },
    };

    let location = match paths::org_plugins_effective() {
        Some(l) => l,
        None => {
            diag("cannot resolve org-plugins directory for this OS");
            return ExitCode::from(1);
        },
    };

    if let Err(e) = bootstrap::bootstrap_directory(&location) {
        diag(&format!("directory bootstrap failed: {e}"));
        return ExitCode::from(1);
    }

    if let Err(e) =
        bootstrap::write_version_sentinel(&location.path, &binary, opts.gateway_url.as_deref())
    {
        diag(&format!("version sentinel write failed: {e}"));
        return ExitCode::from(1);
    }

    if let Some(ref url) = opts.gateway_url {
        if let Err(e) = config::ensure_gateway_url(url) {
            diag(&format!(
                "warning: could not persist gateway_url to config: {e}"
            ));
        }
    }

    if let Some(ref pubkey) = opts.pubkey {
        if let Err(e) = config::persist_pinned_pubkey(pubkey) {
            diag(&format!(
                "warning: failed to persist operator-supplied pubkey to local config: {e}"
            ));
        } else {
            tracing::info!(
                pubkey_len = pubkey.len(),
                "pinned operator-supplied manifest pubkey"
            );
        }
    }

    print_install_summary(&location, &binary);

    let target_os = opts.print_mdm.unwrap_or_else(Os::current);
    let gateway_for_mdm = opts
        .gateway_url
        .clone()
        .or_else(|| config::load().gateway_url)
        .unwrap_or_else(|| "https://gateway.systemprompt.io".into());

    if opts.apply_mobileconfig {
        if let Err(code) = run_apply_mobileconfig(&binary, &gateway_for_mdm, opts.pubkey.as_deref())
        {
            return code;
        }
    } else if opts.apply {
        if let Err(code) = run_apply(target_os, &binary, &gateway_for_mdm, opts.pubkey.as_deref()) {
            return code;
        }
    } else {
        println!();
        println!("--- MDM configuration ({}) ---", mdm::os_label(target_os));
        println!(
            "{}",
            mdm::snippet(target_os, &binary, Some(&gateway_for_mdm))
        );
        println!("Tip: rerun with --apply to write these keys directly.");
    }

    if let Some(schedule_os) = opts.emit_schedule_template {
        if let Err(code) = emit_schedule(schedule_os, &binary) {
            return code;
        }
    }

    ExitCode::SUCCESS
}

fn print_install_summary(location: &paths::OrgPluginsLocation, binary: &std::path::Path) {
    println!("Installed systemprompt-cowork integration");
    println!(
        "  org-plugins: {} ({})",
        location.path.display(),
        match location.scope {
            Scope::System => "system-wide",
            Scope::User => "per-user",
        }
    );
    let meta = paths::metadata_dir(&location.path);
    println!("  metadata:    {}", meta.display());
    println!(
        "    user.json:    {}",
        meta.join(paths::USER_FRAGMENT).display()
    );
    println!(
        "    skills/:      {}",
        meta.join(paths::SKILLS_DIR).display()
    );
    println!(
        "    agents/:      {}",
        meta.join(paths::AGENTS_DIR).display()
    );
    println!(
        "    managed-mcp:  {}",
        meta.join(paths::MANAGED_MCP_FRAGMENT).display()
    );
    println!("  binary:      {}", binary.display());
    println!(
        "  Run `systemprompt-cowork sync` to populate user identity, skills, agents, and MCP \
         servers."
    );
}

#[cfg(target_os = "macos")]
fn run_apply_mobileconfig(
    binary: &std::path::Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<(), ExitCode> {
    match macos::apply_mobileconfig(binary, gateway, pubkey) {
        Ok(summary) => {
            println!();
            println!("--- mobileconfig applied (macOS) ---");
            for line in summary {
                println!("  {line}");
            }
            Ok(())
        },
        Err(e) => {
            diag(&format!("apply --mobileconfig failed: {e}"));
            Err(ExitCode::from(1))
        },
    }
}

#[cfg(not(target_os = "macos"))]
fn run_apply_mobileconfig(
    _binary: &std::path::Path,
    _gateway: &str,
    _pubkey: Option<&str>,
) -> Result<(), ExitCode> {
    diag("--apply-mobileconfig is only supported on macOS");
    Err(ExitCode::from(1))
}

fn run_apply(
    target_os: Os,
    binary: &std::path::Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<(), ExitCode> {
    match mdm::apply_mdm(target_os, binary, gateway, pubkey) {
        Ok(summary) => {
            println!();
            println!("--- policy applied ({}) ---", mdm::os_label(target_os));
            for line in summary {
                println!("  {line}");
            }
            Ok(())
        },
        Err(e) => {
            diag(&format!("apply failed: {e}"));
            Err(ExitCode::from(1))
        },
    }
}

fn emit_schedule(schedule_os: Os, binary: &std::path::Path) -> Result<(), ExitCode> {
    let filename = schedule::template_filename(schedule_os);
    let content = schedule::template(schedule_os, binary);
    let out = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(filename);
    if let Err(e) = fs::write(&out, content) {
        diag(&format!("failed to write {}: {e}", out.display()));
        return Err(ExitCode::from(1));
    }
    println!();
    println!("--- Schedule template ({}) ---", mdm::os_label(schedule_os));
    println!("wrote: {}", out.display());
    println!("{}", schedule::install_hint(schedule_os));
    Ok(())
}

pub fn uninstall(purge: bool) -> ExitCode {
    let location = match paths::org_plugins_effective() {
        Some(l) => l,
        None => {
            diag("cannot resolve org-plugins directory for this OS");
            return ExitCode::from(1);
        },
    };

    let metadata = paths::metadata_dir(&location.path);
    if metadata.exists() {
        if let Err(e) = fs::remove_dir_all(&metadata) {
            diag(&format!(
                "failed to remove metadata dir {}: {e}",
                metadata.display()
            ));
            return ExitCode::from(1);
        }
        println!("Removed {}", metadata.display());
    } else {
        println!("No metadata dir at {} (already clean)", metadata.display());
    }

    let staging = paths::staging_dir(&location.path);
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }

    #[cfg(target_os = "macos")]
    {
        match macos::remove_profile() {
            Ok(true) => println!("Removed managed profile {}", macos::PAYLOAD_IDENTIFIER),
            Ok(false) => println!(
                "No managed profile {} installed (nothing to remove)",
                macos::PAYLOAD_IDENTIFIER
            ),
            Err(e) => diag(&format!("profile remove failed: {e}")),
        }
    }

    if purge {
        match crate::setup::logout() {
            Ok(p) => println!("Purged credentials: {}", p.pat_file.display()),
            Err(e) => diag(&format!("credential purge failed: {e}")),
        }
    } else {
        println!(
            "Credentials left intact. Use `systemprompt-cowork uninstall --purge` to also clear \
             them."
        );
    }
    ExitCode::SUCCESS
}
