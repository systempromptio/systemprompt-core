//! Crate root for the systemprompt bridge: wires the CLI, GUI, proxy, sync, and
//! integration modules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
pub mod basedirs;
pub mod brand;
pub mod buildinfo;
pub mod claude_policy;
pub mod cli;
pub mod config;
pub mod context;
pub mod cowork_compat;
#[cfg(feature = "dev-preview")]
pub mod dev_preview;
pub mod diagnostics_state;
pub mod feedback;
pub mod feedback_capture;
pub mod fsutil;
pub mod gateway;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub mod gui;
pub mod hash;
pub mod host_sync;

pub mod drift;
pub mod headers;
mod help;
pub mod i18n;
pub mod ids;
pub mod install;
pub mod integration;
pub mod last_sync;
pub mod mcp_registry;
pub mod obs;
pub mod probe_cache;
pub mod progress;
pub mod proxy;
pub mod proxy_probe;
pub mod schedule;
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) mod single_instance;
pub mod stdio;
pub mod sync;
pub mod sysproc;
pub mod update;
pub mod user_alert;
pub mod validate;
pub mod verdict;
pub mod web_assets;
pub mod webview2;
pub mod window_state;
#[cfg(target_os = "windows")]
pub(crate) mod winproc;
pub mod wire;

pub(crate) use help::help;
use std::process::ExitCode;

#[must_use]
pub fn run() -> ExitCode {
    run_with_brand(&brand::Brand::SYSTEMPROMPT)
}

// Why: reqwest 0.13 is built with `rustls-no-provider` so the tree carries ring
// only; rustls then needs a process-wide provider before the first client is
// built, or reqwest panics.
fn install_tls_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return;
    }
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("rustls crypto provider was installed concurrently");
    }
}

#[must_use]
pub fn run_with_brand(brand: &'static brand::Brand) -> ExitCode {
    brand::set_brand(brand);
    install_tls_crypto_provider();
    let launch = cli::Launch::detect();
    obs::install_panic_hook();
    if let Err(e) = obs::tracing_init::init() {
        stdio::eprint_str(&format!("bridge startup failed: {e}\n"));
        return ExitCode::FAILURE;
    }
    brand::warn_if_version_drifts();
    await_predecessor_exit();
    update::sweep_leftovers();
    cli::run(launch)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn await_predecessor_exit() {
    single_instance::await_predecessor_exit();
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const fn await_predecessor_exit() {}

pub(crate) mod tasks;

#[cfg(target_os = "windows")]
pub(crate) mod windows_acl;
