//! Claude Code routing checks without exposing settings or credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::Check;
use crate::install::mdm::claude_code_settings::standalone_settings_path;

pub fn check_settings(origin: &str) -> Vec<Check> {
    let mut checks = Vec::new();
    let policy = crate::config::paths::claude_code_policy_dir().join("managed-settings.json");
    let paths = [
        Some(policy),
        crate::config::paths::claude_cli_settings_path(),
        standalone_settings_path(),
    ];
    for path in paths.into_iter().flatten() {
        if path.exists() {
            checks.push(check_file(&path, origin));
        }
    }
    if checks.is_empty() {
        checks.push(Check::warn(
            "claude code settings",
            "not enrolled; run install --host claude-code to create gateway settings",
        ));
    }
    checks
}

pub fn check_file(path: &Path, origin: &str) -> Check {
    let name = "claude code settings";
    let fail = |detail: &str| Check::fail(name, format!("{}: {detail}", path.display()));
    let Ok(body) = std::fs::read_to_string(path) else {
        return fail("cannot read settings");
    };
    let Ok(root) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&body) else {
        return fail("invalid settings JSON");
    };
    if root.contains_key("forceLoginMethod") || root.contains_key("forceLoginOrgUUID") {
        return fail(
            "forced login policy can block gateway authentication; contact your administrator",
        );
    }
    let configured = root
        .get("env")
        .and_then(|env| env.get("ANTHROPIC_BASE_URL"));
    let Some(configured) = configured.and_then(serde_json::Value::as_str) else {
        return Check::warn(
            name,
            format!(
                "{}: no persistent gateway routing; use --settings for a per-session profile",
                path.display()
            ),
        );
    };
    if configured != origin {
        return fail(
            "routing differs from this bridge; re-enrol if this file should use the gateway",
        );
    }
    if root
        .get("apiKeyHelper")
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        return fail("missing credential helper; run install --host claude-code");
    }
    Check::ok(
        name,
        format!("{}: proxy routing and helper configured", path.display()),
    )
}
