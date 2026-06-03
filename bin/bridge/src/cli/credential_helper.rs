#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "git/anthropic credential-helper protocol: secrets are emitted on stdout, \
              diagnostics on stderr"
)]

use std::process::ExitCode;

use systemprompt_identifiers::SessionId;

use crate::auth::ChainError;
use crate::{auth, config, proxy};

pub(super) fn cmd_credential_helper(args: &[String]) -> ExitCode {
    let Some(host) = parse_host(args) else {
        eprintln!("{}", error_json("missing required --host <id>"));
        return ExitCode::from(64);
    };

    match host.as_str() {
        "codex-cli" => emit_codex(),
        "claude-desktop" => emit_claude_via_chain(),
        other => {
            eprintln!("{}", error_json(&format!("unknown host id: {other}")));
            ExitCode::from(64)
        },
    }
}

fn emit_claude_via_chain() -> ExitCode {
    let cfg = config::load();
    let acquired = match proxy::block_on(auth::acquire_bearer(&cfg, &SessionId::generate())) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", error_json(&format!("runtime init failed: {e}")));
            return ExitCode::from(70);
        },
    };
    let out = match acquired {
        Ok(out) => out,
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
                error_json("no credential available; run `systemprompt-bridge login`")
            );
            return ExitCode::from(5);
        },
    };
    emit_claude(&out)
}

fn emit_codex() -> ExitCode {
    // Codex authenticates against the local loopback proxy, not the upstream
    // gateway: the helper must hand it the loopback secret, not a gateway bearer.
    let secret = match proxy::secret::for_profile() {
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
    // Codex stuffs the helper's stdout verbatim into `Authorization: Bearer
    // <stdout>` without parsing JSON, so the bare secret (not an envelope) must
    // be printed.
    println!("{}", secret.as_str());
    ExitCode::SUCCESS
}

fn emit_claude(out: &auth::types::HelperOutput) -> ExitCode {
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

fn parse_host(args: &[String]) -> Option<String> {
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

fn error_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
