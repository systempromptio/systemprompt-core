//! `login` command: stores a PAT, pasted directly or redeemed from an
//! admin-issued one-shot exchange code.
//!
//! Despite the name this opens no browser and runs no device-link flow. Two
//! ways in: paste a `sp-live-…` token, or pass `--code` from
//! `admin bridge issue-code`, which this redeems for a durable PAT against the
//! same `/v1/auth/bridge/session-pat` endpoint the desktop GUI uses — the
//! headless equivalent of the tray sign-in, and the only browserless way to
//! bootstrap on Linux, where there is no GUI.
//!
//! Device-link lives in `auth::providers::session`, and is interactive per
//! authentication, so it cannot carry the proxy: that re-authenticates per
//! request and would reopen a browser on every hook. A device certificate
//! (`admin bridge enroll-cert`) is the only credential that renews unattended.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_identifiers::{SessionId, ValidatedUrl};

use crate::auth::setup;
use crate::auth::types::SessionPatRequest;
use crate::cli::args::parse_opt_flag;
use crate::cli::output;
use crate::gateway::GatewayClient;
use crate::obs::output::diag;

pub fn cmd_login(args: &[String]) -> ExitCode {
    let gateway = parse_opt_flag(args, "--gateway");

    // A code and a pasted PAT are two routes to the same stored credential, so
    // they converge before setup::login.
    let token = if let Some(code) = parse_opt_flag(args, "--code") {
        let device_name = parse_opt_flag(args, "--device-name");
        match redeem_code(&code, gateway.as_deref(), device_name) {
            Ok(pat) => pat,
            Err(e) => {
                diag(&format!("login: could not redeem the exchange code: {e}"));
                return ExitCode::from(1);
            },
        }
    } else {
        let Some(t) = args.get(2).filter(|t| !t.is_empty() && !t.starts_with('-')) else {
            diag(&usage());
            return ExitCode::from(64);
        };
        t.clone()
    };

    match setup::login(&token, gateway.as_deref()) {
        Ok(paths) => {
            let bin = crate::brand::brand().binary_name;
            output::print_line(&format!("Stored PAT for {bin} helper."));
            output::print_line(&format!("  config: {}", paths.config_file.display()));
            output::print_line(&format!("  secret: {} (0600)", paths.pat_file.display()));
            output::print_line(&format!("Next: run `{bin}` to fetch a JWT."));
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("login failed: {e}"));
            ExitCode::from(1)
        },
    }
}

fn usage() -> String {
    format!(
        "usage: {bin} login <sp-live-...> [--gateway <url>]\n   \
         or: {bin} login --code <exchange-code> [--gateway <url>] [--device-name <name>]\n\
         \n\
         An administrator issues a code with:\n  \
         systemprompt admin bridge issue-code --user-id <uuid>",
        bin = crate::brand::brand().binary_name
    )
}

/// Trades a one-shot exchange code for a durable PAT.
fn redeem_code(
    code: &str,
    gateway: Option<&str>,
    device_name: Option<String>,
) -> Result<String, String> {
    let cfg = crate::config::load();
    let base_url = match gateway {
        Some(raw) => ValidatedUrl::try_new(raw.trim()).map_err(|e| format!("--gateway: {e}"))?,
        None => crate::config::gateway_url_or_default(&cfg),
    };
    let req = SessionPatRequest {
        code: code.trim().to_owned(),
        device_name: device_name.or_else(default_device_name),
    };
    let client = GatewayClient::new(base_url);
    crate::proxy::block_on(async move {
        client
            .session_pat_exchange(&req, &SessionId::generate())
            .await
    })
    .map_err(|e| format!("runtime init: {e}"))?
    .map_err(|e| e.to_string())
}

/// Labels the PAT in the admin's device list, so a revoke can name a machine.
fn default_device_name() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|h| h.trim().to_owned())
                .filter(|h| !h.is_empty())
        })
}
