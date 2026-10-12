//! The `claude -p` argument vector and child environment for one run.
//!
//! The environment is the complete set the adapter asks for: the run's home,
//! the gateway, the run's PAT and gateway session, and the switches that keep
//! Claude Code from phoning home. The executor decides what else (`PATH`,
//! `LANG`) the child inherits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::plugin::HostFiles;
use crate::host::PrepareContext;
use crate::request::{PermissionMode, RunRequest};

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn push_pair(args: &mut Vec<String>, flag: &str, value: String) {
    args.push(flag.to_owned());
    args.push(value);
}

#[must_use]
pub fn appended_system_prompt(request: &RunRequest) -> Option<String> {
    let parts: Vec<&str> = [
        request.persona.as_deref(),
        request.system_prompt_append.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.trim().is_empty())
    .collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

#[must_use]
pub fn claude_args(request: &RunRequest, files: &HostFiles) -> Vec<String> {
    let mut args = vec![
        "-p".to_owned(),
        request.prompt.clone(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--verbose".to_owned(),
    ];
    if let Some(model) = request.model.as_deref().filter(|m| !m.trim().is_empty()) {
        push_pair(&mut args, "--model", model.to_owned());
    }
    permission_args(&mut args, request);
    if !request.allowed_tools.is_empty() {
        push_pair(&mut args, "--allowedTools", request.allowed_tools.join(","));
    }
    if !request.disallowed_tools.is_empty() {
        push_pair(
            &mut args,
            "--disallowedTools",
            request.disallowed_tools.join(","),
        );
    }
    if let Some(prompt) = appended_system_prompt(request) {
        push_pair(&mut args, "--append-system-prompt", prompt);
    }
    if let Some(turns) = request.max_turns {
        push_pair(&mut args, "--max-turns", turns.max(1).to_string());
    }
    if let Some(usd) = request.max_budget_usd {
        push_pair(
            &mut args,
            "--max-budget-usd",
            format!("{:.2}", usd.max(0.01)),
        );
    }
    for dir in &request.add_dirs {
        push_pair(&mut args, "--add-dir", path_arg(dir));
    }
    if let Some(mcp) = &files.mcp_config {
        push_pair(&mut args, "--mcp-config", path_arg(mcp));
        args.push("--strict-mcp-config".to_owned());
    }
    if let Some(schema) = &request.output_schema {
        push_pair(&mut args, "--json-schema", schema.to_string());
    }
    if let Some(session) = &request.resume_session_id {
        push_pair(&mut args, "--resume", session.as_str().to_owned());
    }
    for dir in request
        .plugin_dirs
        .iter()
        .chain(files.governance_plugin.iter())
    {
        push_pair(&mut args, "--plugin-dir", path_arg(dir));
    }
    args
}

fn permission_args(args: &mut Vec<String>, request: &RunRequest) {
    if request.permission_prompts_none
        && request.permission_mode == PermissionMode::BypassPermissions
    {
        args.push("--dangerously-skip-permissions".to_owned());
    } else {
        push_pair(
            args,
            "--permission-mode",
            request.permission_mode.as_str().to_owned(),
        );
    }
}

#[must_use]
pub fn child_env(ctx: &PrepareContext<'_>) -> Vec<(String, String)> {
    let home = path_arg(&ctx.layout.home);
    let base = ctx.gateway_base_url.trim_end_matches('/');
    let creds = ctx.credentials;
    let mut vars = vec![
        ("HOME", home.clone()),
        ("CLAUDE_CONFIG_DIR", format!("{home}/.claude")),
        ("TERM", "dumb".to_owned()),
        ("ANTHROPIC_BASE_URL", base.to_owned()),
        ("ANTHROPIC_AUTH_TOKEN", creds.pat.as_str().to_owned()),
        ("ANTHROPIC_CUSTOM_HEADERS", custom_headers(ctx)),
        ("SYSTEMPROMPT_API_URL", base.to_owned()),
        ("DISABLE_AUTOUPDATER", "1".to_owned()),
        ("DISABLE_TELEMETRY", "1".to_owned()),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_owned()),
    ];
    if let Some(token) = &creds.hook_token {
        vars.push(("SYSTEMPROMPT_TOKEN", token.clone()));
    }
    vars.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

// Claude Code reads `ANTHROPIC_CUSTOM_HEADERS` as newline-separated
// `Name: value` lines and sends each on every API call.
fn custom_headers(ctx: &PrepareContext<'_>) -> String {
    let session = format!(
        "x-session-id: {}",
        ctx.credentials.gateway_session_id.as_str()
    );
    std::iter::once(session)
        .chain(
            ctx.request
                .scope_headers()
                .into_iter()
                .map(|(name, value)| format!("{name}: {value}")),
        )
        .collect::<Vec<_>>()
        .join("\n")
}
