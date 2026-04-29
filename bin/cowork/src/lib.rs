pub mod auth;
pub mod cli;
pub mod config;
pub mod gateway;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod gui;
pub mod http_local;
pub mod ids;
pub mod install;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod integration;
pub mod obs;
pub mod proxy;
pub mod schedule;
pub mod sync;
pub mod validate;
#[cfg(target_os = "windows")]
pub(crate) mod winproc;

use std::process::ExitCode;

const HELP: &str = "systemprompt-cowork <command>

Commands (credential helper):
  run                        (default) Emit JWT envelope to stdout
  login <sp-live-...>        Store a PAT securely and wire up systemprompt-cowork.toml
    [--gateway <url>]
  logout                     Remove the stored PAT and its config section
  clean                      Wipe all local cowork state (config + PAT + token cache).
                             Returns the GUI to a fresh splash. Does not touch
                             org-plugins or managed profiles — see `uninstall --purge`.
  status                     Show config paths and what is currently set up
  whoami                     Print authenticated identity from the gateway

Commands (plugin + MCP sync):
  install                    Bootstrap Cowork integration on this machine
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
  validate                   End-to-end self-check (paths, gateway, creds, signatures)
  uninstall                  Reverse install (metadata + staging)
    [--purge]                             Also remove stored PAT/credentials
  gui                        Launch the native settings UI (Windows + macOS)
  help                       Show this help

Env overrides:
  SP_COWORK_CONFIG           Path to systemprompt-cowork.toml
  SP_COWORK_PAT              Inline PAT (overrides file-based [pat])
  SP_COWORK_GATEWAY_URL      Override gateway_url
";

pub(crate) fn help() -> &'static str {
    HELP
}

pub fn run() -> ExitCode {
    #[cfg(target_os = "windows")]
    winproc::attach_parent_console_if_present();
    obs::tracing_init::init();
    cli::run()
}
