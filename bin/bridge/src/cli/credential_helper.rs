//! Credential-helper mode: emits Claude/Codex credentials on stdout for host
//! apps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "git/anthropic credential-helper protocol: secrets are emitted on stdout, \
              diagnostics on stderr"
)]

use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::ChainError;
use crate::context::BridgeContext;
use crate::{auth, config};

pub(super) fn cmd_credential_helper(ctx: &BridgeContext, args: &[String]) -> ExitCode {
    let Some(host) = parse_host(args) else {
        eprintln!("{}", error_json("missing required --host <id>"));
        return ExitCode::from(64);
    };

    match host.as_str() {
        "codex-cli" => emit_codex(ctx),
        "claude-desktop" => emit_claude_via_chain(ctx),
        other => {
            eprintln!("{}", error_json(&format!("unknown host id: {other}")));
            ExitCode::from(64)
        },
    }
}

fn emit_claude_via_chain(ctx: &BridgeContext) -> ExitCode {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            crate::stdio::diag(&e.to_string());
            return ExitCode::FAILURE;
        },
    };
    let acquired = ctx.block_on(auth::acquire_bearer(
        &cfg,
        &SessionId::generate(),
        &ctx.http,
    ));
    let out = match acquired {
        Ok(out) => out,
        Err(e @ ChainError::Providers(_)) => {
            crate::stdio::diag(&format!("{e}"));
            return ExitCode::FAILURE;
        },
        Err(ChainError::Cache(e)) => {
            crate::stdio::diag(&format!("credential cache: {e}"));
            return ExitCode::FAILURE;
        },
        Err(ChainError::PreferredTransient { provider, source }) => {
            eprintln!(
                "{}",
                error_json(&format!("transient auth failure on {provider}: {source}"))
            );
            return ExitCode::from(10);
        },
        Err(ChainError::NoneSucceeded) => {
            eprintln!(
                "{}",
                error_json(&format!(
                    "no credential available; run `{} login`",
                    crate::brand::brand().binary_name
                ))
            );
            return ExitCode::from(5);
        },
    };
    emit_claude(&out)
}

fn emit_codex(ctx: &BridgeContext) -> ExitCode {
    let secret = match ctx.proxy.loopback().secret() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{}",
                error_json(&format!(
                    "loopback secret unavailable: {e}; start the bridge once to mint it"
                ))
            );
            return ExitCode::from(70);
        },
    };
    // Why: Codex forwards helper stdout as the bearer credential, so it must be a
    // bare secret.
    println!("{}", secret.as_str());
    ExitCode::SUCCESS
}

fn emit_claude(out: &crate::gateway::types::HelperOutput) -> ExitCode {
    match serde_json::to_string(out) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        },
        Err(e) => {
            eprintln!("{}", error_json(&format!("serialize failed: {e}")));
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

pub fn error_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
