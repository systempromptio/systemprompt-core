//! Credential-helper mode: the Codex `auth.command` / Claude Code
//! `apiKeyHelper` contract.
//!
//! The credential is the only line on stdout; diagnostics are a JSON object
//! on stderr. A CLI host receives the token derived for it from the loopback
//! secret, never the secret itself. A host that owns its configuration
//! receives nothing: naming it is a usage error, like an unknown id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::ExitCode;

use systemprompt_models::bridge::host::{ConfigOwner, HostKind, UnknownHostKind};

use crate::context::BridgeContext;
use crate::proxy::scoped_token::HostOwnsConfig;
use crate::{auth, config, stdio};

/// The credential the helper prints for a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperCredential {
    GatewayChain,
    HostToken(HostKind),
}

/// Why `--host <id>` names no host the helper serves.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HelperHostError {
    #[error(transparent)]
    Unknown(#[from] UnknownHostKind),
    #[error(transparent)]
    HostOwned(#[from] HostOwnsConfig),
}

pub fn credential_for(host: &str) -> Result<HelperCredential, HelperHostError> {
    let kind = host.parse::<HostKind>()?;
    if kind.config_owner() != ConfigOwner::Bridge {
        return Err(HostOwnsConfig(kind).into());
    }
    Ok(if kind == HostKind::ClaudeDesktop {
        HelperCredential::GatewayChain
    } else {
        HelperCredential::HostToken(kind)
    })
}

pub(super) fn cmd_credential_helper(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let Some(host) = parse_host(args) else {
        stdio::eprint_line(&error_json("missing required --host <id>"));
        return ExitCode::from(64);
    };

    match credential_for(&host) {
        Ok(HelperCredential::GatewayChain) => emit_claude_via_chain(ctx),
        Ok(HelperCredential::HostToken(kind)) => emit_host_token(ctx, kind),
        Err(e) => {
            stdio::eprint_line(&error_json(&e.to_string()));
            ExitCode::from(64)
        },
    }
}

fn emit_claude_via_chain(ctx: &BridgeContext) -> ExitCode {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            stdio::diag(&e.to_string());
            return ExitCode::FAILURE;
        },
    };
    let acquired = ctx.block_on(auth::acquire_bearer(
        &cfg,
        &auth::session_identity::stable_session_id(&cfg),
        &ctx.http,
    ));
    let out = match acquired {
        Ok(out) => out,
        Err(e) => {
            let (code, message) = e.exit_report();
            stdio::eprint_line(&error_json(&message));
            return code;
        },
    };
    emit_claude(&out)
}

fn emit_host_token(ctx: &BridgeContext, host: HostKind) -> ExitCode {
    let secret = match ctx.proxy.loopback().secret() {
        Ok(s) => s,
        Err(e) => {
            stdio::eprint_line(&error_json(&format!(
                "loopback secret unavailable: {e}; start the bridge once to mint it"
            )));
            return ExitCode::from(70);
        },
    };
    // Why: CLI clients forward helper stdout as the bearer credential, so it must
    // be a bare token.
    match crate::proxy::scoped_token::host_token(&secret, host) {
        Ok(token) => {
            stdio::print_line(token.as_str());
            ExitCode::SUCCESS
        },
        Err(e) => {
            stdio::eprint_line(&error_json(&e.to_string()));
            ExitCode::from(64)
        },
    }
}

fn emit_claude(out: &crate::gateway::types::HelperOutput) -> ExitCode {
    match serde_json::to_string(&out.for_host()) {
        Ok(s) => {
            stdio::print_line(&s);
            ExitCode::SUCCESS
        },
        Err(e) => {
            stdio::eprint_line(&error_json(&format!("serialize failed: {e}")));
            ExitCode::from(3)
        },
    }
}

pub fn parse_host(args: &[String]) -> Option<String> {
    let mut iter = args.iter().skip(2);
    while let Some(arg) = iter.next() {
        if arg == "--host" {
            return iter.next().cloned();
        }
        if let Some(rest) = arg.strip_prefix("--host=") {
            return Some(rest.to_owned());
        }
    }
    None
}

// JSON: helper stderr contract — one object with an `error` string.
pub fn error_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
