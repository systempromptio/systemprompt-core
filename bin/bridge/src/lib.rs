#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::similar_names
)]

pub mod activity;
pub mod auth;
pub mod cli;
pub mod config;
pub mod gateway;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod gui;

#[cfg(all(
    feature = "ts-export",
    not(any(target_os = "windows", target_os = "macos"))
))]
#[path = "gui/ipc.rs"]
pub mod ipc_types;
pub mod i18n;
pub mod ids;
pub mod install;
pub mod integration;
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

const HELP: &str = "systemprompt-bridge <command>

Commands (credential helper):
  run                        (default) Emit JWT envelope to stdout
  login <sp-live-...>        Store a PAT securely and wire up systemprompt-bridge.toml
    [--gateway <url>]
  logout                     Remove the stored PAT and its config section
  clean                      Wipe all local systemprompt-bridge state (config + PAT + token cache).
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
  SP_BRIDGE_CONFIG           Path to systemprompt-bridge.toml
  SP_BRIDGE_PAT              Inline PAT (overrides file-based [pat])
  SP_BRIDGE_GATEWAY_URL      Override gateway_url
";

#[must_use]
pub(crate) const fn help() -> &'static str {
    HELP
}

#[must_use]
pub fn run() -> ExitCode {
    #[cfg(target_os = "windows")]
    winproc::attach_parent_console_if_present();
    obs::install_panic_hook();
    obs::tracing_init::init();
    activity::install_persistent_writer();
    cli::run()
}

#[cfg(all(test, feature = "ts-export"))]
mod ts_export_tests {
    #![allow(clippy::expect_used)]

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, IpcRequest};
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    use crate::ipc_types::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, IpcRequest};
    use ts_rs::TS;

    #[test]
    #[ignore]
    fn export_bindings() {
        assert!(
            std::env::var_os("TS_RS_EXPORT_DIR").is_some(),
            "TS_RS_EXPORT_DIR must be set so ts-rs writes paths relative to the crate root. \
             Run: TS_RS_EXPORT_DIR=. cargo test --features ts-export export_bindings -- --ignored"
        );
        BridgeError::export_all().expect("export BridgeError");
        ErrorScope::export_all().expect("export ErrorScope");
        ErrorCode::export_all().expect("export ErrorCode");
        IpcRequest::export_all().expect("export IpcRequest");
        IpcReplyPayload::export_all().expect("export IpcReplyPayload");
    }
}
