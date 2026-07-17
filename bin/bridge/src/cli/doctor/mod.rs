//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use crate::cli::output;
use crate::{config, obs};

pub mod auth;
pub mod cowork;
pub mod filesystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

impl Check {
    pub fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
        }
    }
    pub fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Warn,
            detail: detail.into(),
        }
    }
    pub fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Fail,
            detail: detail.into(),
        }
    }
}

pub(super) fn cmd_doctor() -> ExitCode {
    let result = crate::proxy::block_on(async { run_checks().await });
    match result {
        Ok((checks, any_fail)) => {
            render(&checks);
            if any_fail {
                ExitCode::from(11)
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(e) => {
            obs::output::diag(&format!("doctor: runtime init failed: {e}"));
            ExitCode::from(70)
        },
    }
}

pub async fn run_checks() -> (Vec<Check>, bool) {
    let cfg = config::load();
    let mut checks: Vec<Check> = Vec::new();
    checks.push(auth::check_config_file());
    checks.push(auth::check_credential_source(&cfg));
    let bearer = auth::check_mint_jwt(&cfg, &mut checks).await;
    let client = auth::check_gateway_reachable(&cfg, &mut checks).await;
    auth::check_whoami(&client, bearer.as_ref(), &mut checks).await;
    checks.push(auth::check_loopback_secret());
    if let Some(check) = auth::check_host_profile_secrets() {
        checks.push(check);
    }
    checks.push(auth::check_pinned_pubkey());
    checks.push(cowork::check_cowork_enable());
    checks.push(cowork::check_plugin_installation_preference());
    checks.push(cowork::check_personal_session_sentinel());
    checks.push(filesystem::check_bridge_working_dir());
    checks.push(filesystem::check_org_plugins_writable());
    checks.push(auth::check_hook_token_mint(&client).await);
    let any_fail = checks.iter().any(|c| matches!(c.status, Status::Fail));
    (checks, any_fail)
}

fn render(checks: &[Check]) {
    let mut buf = String::new();
    for c in checks {
        let tag = match c.status {
            Status::Ok => "OK  ",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
        };
        buf.push_str(&format!("[{tag}] {:<28} {}\n", c.name, c.detail));
    }
    let fails = checks
        .iter()
        .filter(|c| matches!(c.status, Status::Fail))
        .count();
    let warns = checks
        .iter()
        .filter(|c| matches!(c.status, Status::Warn))
        .count();
    buf.push_str(&format!(
        "\nsummary: {} ok, {warns} warn, {fails} fail\n",
        checks.len() - fails - warns
    ));
    output::print_str(&buf);
}
