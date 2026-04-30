use std::process::ExitCode;

use chrono::{SecondsFormat, Utc};

use crate::auth::ChainError;
use crate::{auth, config};

pub(crate) fn cmd_credential_helper(args: &[String]) -> ExitCode {
    let host = parse_host(args);
    let host = match host {
        Some(h) => h,
        None => {
            eprintln!("{}", error_json("missing required --host <id>"));
            return ExitCode::from(64);
        },
    };

    let cfg = config::load();
    let out = match auth::acquire_bearer(&cfg) {
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

    match host.as_str() {
        "claude-desktop" => emit_claude(&out),
        "codex-cli" => emit_codex(&out),
        other => {
            eprintln!("{}", error_json(&format!("unknown host id: {other}")));
            return ExitCode::from(64);
        },
    }
}

fn emit_codex(out: &auth::types::HelperOutput) -> ExitCode {
    let expires_at = expires_at_rfc3339(out.ttl);
    let body = serde_json::json!({
        "token": out.token.expose(),
        "expires_at": expires_at,
    });
    match serde_json::to_string(&body) {
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
            return Some(rest.to_string());
        }
    }
    None
}

fn expires_at_rfc3339(ttl_secs: u64) -> Option<String> {
    if ttl_secs == 0 {
        return None;
    }
    let expiry = Utc::now() + chrono::Duration::seconds(ttl_secs as i64);
    Some(expiry.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn error_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}
