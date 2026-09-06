//! CLI command tree for the bridge binary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod args;
pub mod clean;
pub mod comms_drain;
pub mod credential_helper;
#[cfg(feature = "dev-preview")]
mod dev_web;
pub mod diagnostics;
pub mod doctor;
mod gui;
mod install;
mod install_claude_policy;
pub mod login;
pub mod logout;
pub mod oauth_client;
pub mod proxy;
mod run;
pub mod status;
pub mod sync;
mod uninstall;
pub mod update;
pub mod validate;
pub mod whoami;

use std::env;
use std::process::ExitCode;
use std::sync::Arc;

use crate::context::{BridgeContext, ProxyMode};
use crate::stdio::{self, diag};

pub fn run() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    run_with_args(&args)
}

pub fn run_with_args(args: &[String]) -> ExitCode {
    let command = args.get(1).map(String::as_str);
    match command {
        Some("--version" | "-V" | "version") => {
            stdio::print_str(&format!(
                "{} {} ({}, {})\n",
                crate::brand::brand().binary_name,
                crate::brand::brand().version,
                diagnostics::short_sha(),
                diagnostics::GIT_COMMIT_DATE,
            ));
            return ExitCode::SUCCESS;
        },
        Some("help" | "--help" | "-h") => {
            stdio::print_str(&crate::help());
            return ExitCode::SUCCESS;
        },
        _ => {},
    }

    let default_gui = args.len() == 1 && args::should_default_to_gui();
    // Why: only the long-running commands own the loopback port. Everything
    // else (`install --apply`, `sync`, `doctor`, the credential helpers) runs
    // beside a bridge that is already serving and must find it, not race it.
    let mode = if default_gui || matches!(command, Some("proxy" | "gui")) {
        ProxyMode::Serve
    } else {
        ProxyMode::Attach
    };
    let ctx = match BridgeContext::start(mode) {
        Ok(ctx) => ctx,
        Err(e) => {
            diag(&format!("runtime init failed: {e}"));
            return ExitCode::from(70);
        },
    };
    if default_gui {
        return gui::cmd_gui(ctx);
    }
    dispatch(command, args, ctx)
}

fn dispatch(command: Option<&str>, args: &[String], ctx: Arc<BridgeContext>) -> ExitCode {
    match command {
        None | Some("run") => run::cmd_run(&ctx),
        Some("proxy") => proxy::cmd_proxy(&ctx),
        Some("login") => login::cmd_login(&ctx, args),
        Some("logout") => logout::cmd_logout(),
        Some("clean") => clean::cmd_clean(),
        Some("status") => status::cmd_status(),
        Some("whoami") => whoami::cmd_whoami(&ctx),
        Some("install") => install::cmd_install(&ctx, args),
        Some("__install-claude-policy") => install_claude_policy::cmd(args),
        Some("sync") => sync::cmd_sync(&ctx, args),
        Some("update") => update::cmd_update(&ctx, args),
        Some("oauth-client") => oauth_client::cmd_oauth_client(&ctx, args),
        Some("validate") => validate::cmd_validate(&ctx),
        Some("uninstall") => uninstall::cmd_uninstall(&ctx, args),
        Some("credential-helper") => credential_helper::cmd_credential_helper(&ctx, args),
        Some("comms-drain") => comms_drain::cmd_comms_drain(),
        Some("diagnostics") => diagnostics::cmd_diagnostics(),
        Some("doctor") => doctor::cmd_doctor(&ctx),
        Some("gui") => gui::cmd_gui(ctx),
        #[cfg(feature = "dev-preview")]
        Some("dev-web") => dev_web::cmd_dev_web(args),
        Some(other) => {
            diag(&format!("unknown command: {other}"));
            stdio::eprint_str(&crate::help());
            ExitCode::from(64)
        },
    }
}
