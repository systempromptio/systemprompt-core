#![expect(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    reason = "bridge crate-wide pedantic carve-outs: docs are tracked in CHANGELOG; module \
              repetition is structural; similar names are platform-paired (e.g., \
              macos_*/windows_*)"
)]
#![allow(
    clippy::missing_panics_doc,
    reason = "fulfilled only when the macos/windows GUI modules compile, so expect would fail \
              on Linux builds"
)]

pub mod activity;
pub mod auth;
pub mod brand;
pub mod cli;
pub mod config;
pub mod fsutil;
pub mod gateway;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod gui;

pub mod i18n;
pub mod ids;
pub mod install;
pub mod integration;
#[cfg(all(
    feature = "ts-export",
    not(any(target_os = "windows", target_os = "macos"))
))]
#[path = "gui/ipc.rs"]
pub mod ipc_types;
pub mod mcp_registry;
pub mod obs;
pub mod proxy;
pub mod schedule;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) mod single_instance;
pub mod sync;
pub(crate) mod sysproc;
pub mod validate;
#[cfg(target_os = "windows")]
pub(crate) mod winproc;

use std::process::ExitCode;

#[must_use]
pub(crate) fn help() -> String {
    let b = brand::brand();
    let bin = b.binary_name;
    format!(
        "{bin} <command>

Commands (credential helper):
  run                        (default) Emit JWT envelope to stdout
  proxy                      Run the local inference proxy headlessly (Linux/server
                             equivalent of the desktop GUI). Listens on
                             127.0.0.1:48217, swaps a loopback secret for a fresh
                             gateway JWT, injects identity headers, and refreshes
                             in the background. Point ANTHROPIC_BASE_URL /
                             ANTHROPIC_AUTH_TOKEN at the printed values.
  login <sp-live-...>        Store a PAT securely and wire up {config_file}
    [--gateway <url>]
  logout                     Remove the stored PAT and its config section
  clean                      Wipe all local {bin} state (config + PAT + token cache).
                             Returns the GUI to a fresh splash. Does not touch
                             org-plugins or managed profiles — see `uninstall --purge`.
  status                     Show config paths and what is currently set up
  whoami                     Print authenticated identity from the gateway

Commands (plugin + MCP sync):
  install                    Bootstrap Bridge integration on this machine
    [--gateway <url>]                     Persist gateway URL
    [--pubkey <base64>]                   Pin manifest signing pubkey out of band.
                                          With --apply, also written to
                                          HKCU\\SOFTWARE\\Policies\\Claude
                                          (Windows) or the Managed Preferences plist
                                          (macOS) so MDM can roll it to a fleet.
    [--apply]                             Apply locally (Windows registry / macOS
                                          Managed Preferences direct-write). No MDM
                                          needed — works for a single-user dev setup.
    [--apply-mobileconfig]                (macOS) Build .mobileconfig and open System
                                          Settings → Profiles for user approval.
                                          Use when the fleet is MDM-managed or Apple's
                                          approval UI is required.
    [--print-mdm macos|windows|linux]     Print MDM snippet for target OS (default: current OS)
    [--emit-schedule-template macos|windows|linux]
                                          Write an OS scheduler template to CWD
  sync                       Pull plugins + MCP allowlist from gateway into org-plugins
    [--watch] [--interval <secs>] [--allow-unsigned] [--force-replay] [--allow-tofu]
                                          --allow-tofu opts back into trust-on-first-use
                                          pubkey fetch when no pinned key is available;
                                          required only if MDM rollout is unavailable.
  oauth-client               Manage the per-tenant OAuth client used to mint
                             plugin-scoped hook tokens
    status                              Show locally-stashed creds (no secret echo)
    rotate                              Force re-provision; new client_secret minted
  validate                   End-to-end self-check (paths, gateway, creds, signatures)
  doctor                     Diagnose common bridge failure modes (config, creds, gateway,
                             loopback secret, pinned pubkey) with one line per check
  uninstall                  Reverse install (metadata + staging)
    [--purge]                             Also remove stored PAT/credentials
  gui                        Launch the native settings UI (Windows + macOS)
  help                       Show this help

Env overrides:
  {config_env}           Path to {config_file}
  {pat_env}              Inline PAT (overrides file-based [pat])
  {gateway_env}      Override gateway_url
",
        config_file = b.config_file,
        config_env = b.env("CONFIG"),
        pat_env = b.env("PAT"),
        gateway_env = b.env("GATEWAY_URL"),
    )
}

/// Entry point for the default systemprompt-branded binary.
#[must_use]
pub fn run() -> ExitCode {
    run_with_brand(&brand::Brand::SYSTEMPROMPT)
}

/// Entry point for white-label binaries: install the supplied brand, then run.
///
/// The brand is installed *before* any logging, panic-hook, or path resolution
/// so that on-disk directories and chrome reflect the brand from the first
/// line of output. Must be called once at process start, before [`run`].
#[must_use]
pub fn run_with_brand(brand: &'static brand::Brand) -> ExitCode {
    brand::set_brand(brand);
    #[cfg(target_os = "windows")]
    winproc::attach_parent_console_if_present();
    obs::install_panic_hook();
    obs::tracing_init::init();
    activity::install_persistent_writer();
    purge_legacy_agents_state();
    cli::run()
}

fn purge_legacy_agents_state() {
    let Some(base) = dirs::config_dir() else {
        return;
    };
    let path = base.join(brand::brand().config_dir).join("agents.json");
    match std::fs::remove_file(&path) {
        Ok(()) => tracing::info!(path = %path.display(), "purged legacy agents state file"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "purge legacy agents state failed");
        },
    }
}
