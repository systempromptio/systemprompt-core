//! The run's `CODEX_HOME/config.toml`.
//!
//! Routes every model call through the gateway as the `systemprompt`
//! provider (`wire_api = "responses"`, PAT read from [`PAT_ENV`], the run's
//! gateway session in `x-session-id`), never asks for approval, and maps
//! [`PermissionMode`] onto a sandbox. Every key here was accepted by Codex
//! CLI 0.157.0 under `--strict-config`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::PAT_ENV;
use crate::credentials::RunCredentials;
use crate::request::{McpServerSpec, PermissionMode, RunRequest};

pub const CLIENT_HEADER: &str = "headless-codex";

#[must_use]
pub const fn sandbox_mode(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Plan => "read-only",
        PermissionMode::Default | PermissionMode::AcceptEdits => "workspace-write",
        PermissionMode::BypassPermissions => "danger-full-access",
    }
}

#[must_use]
pub fn render(
    request: &RunRequest,
    credentials: &RunCredentials,
    gateway_base_url: &str,
) -> String {
    let mut out = String::new();
    line(&mut out, "model_provider", &quote("systemprompt"));
    line(&mut out, "approval_policy", &quote("never"));
    line(
        &mut out,
        "sandbox_mode",
        &quote(sandbox_mode(request.permission_mode)),
    );
    if !request.add_dirs.is_empty() {
        let roots: Vec<String> = request
            .add_dirs
            .iter()
            .map(|d| d.display().to_string())
            .collect();
        out.push_str("\n[sandbox_workspace_write]\n");
        line(&mut out, "writable_roots", &array(&roots));
    }
    out.push_str("\n[model_providers.systemprompt]\n");
    line(&mut out, "name", &quote("systemprompt"));
    let base = format!("{}/v1", gateway_base_url.trim_end_matches('/'));
    line(&mut out, "base_url", &quote(&base));
    line(&mut out, "wire_api", &quote("responses"));
    line(&mut out, "env_key", &quote(PAT_ENV));
    let mut headers = BTreeMap::from([
        (
            "x-session-id".to_owned(),
            credentials.gateway_session_id.as_str().to_owned(),
        ),
        ("x-systemprompt-client".to_owned(), CLIENT_HEADER.to_owned()),
    ]);
    headers.extend(request.scope_headers());
    line(&mut out, "http_headers", &table(&headers));
    for (name, spec) in &request.mcp_servers {
        out.push_str(&format!("\n[mcp_servers.{}]\n", quote(name)));
        match spec {
            McpServerSpec::Stdio { command, args, env } => {
                line(&mut out, "command", &quote(command));
                line(&mut out, "args", &array(args));
                if !env.is_empty() {
                    line(&mut out, "env", &table(env));
                }
            },
            McpServerSpec::Http { url, headers } => {
                line(&mut out, "url", &quote(url));
                if !headers.is_empty() {
                    line(&mut out, "http_headers", &table(headers));
                }
            },
        }
    }
    out
}

fn line(out: &mut String, key: &str, value: &str) {
    out.push_str(&format!("{key} = {value}\n"));
}

#[must_use]
pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04X}", u32::from(c)));
            },
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn array(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| quote(item)).collect();
    format!("[{}]", quoted.join(", "))
}

fn table(entries: &BTreeMap<String, String>) -> String {
    let pairs: Vec<String> = entries
        .iter()
        .map(|(key, value)| format!("{} = {}", quote(key), quote(value)))
        .collect();
    format!("{{ {} }}", pairs.join(", "))
}
