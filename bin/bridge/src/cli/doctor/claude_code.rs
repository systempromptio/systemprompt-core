//! Claude Code routing checks without exposing settings or credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::Check;
use crate::install::mdm::claude_code_settings::standalone_settings_path;

const CONFIG_DIR_VAR: &str = "CLAUDE_CONFIG_DIR";

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
    let config_dir = std::env::var_os(CONFIG_DIR_VAR).map(PathBuf::from);
    checks.push(check_effective_routing(&read_paths(config_dir.as_deref())));
    checks.extend(config_dir.as_deref().map(check_config_dir_override));
    checks.extend(check_env_credentials(|key| std::env::var_os(key).is_some()));
    checks
}

pub fn read_paths(config_dir: Option<&Path>) -> Vec<PathBuf> {
    let policy = crate::config::paths::claude_code_policy_dir().join("managed-settings.json");
    let user = config_dir.map_or_else(crate::config::paths::claude_cli_settings_path, |dir| {
        Some(dir.join("settings.json"))
    });
    std::iter::once(policy).chain(user).collect()
}

// Why: a user upgraded past an enrolment that no longer applies sees only a
// login prompt; the per-file checks pass for files Claude Code never reads.
pub fn check_effective_routing(paths: &[PathBuf]) -> Check {
    let name = "claude code routing";
    let docs: Vec<serde_json::Map<String, serde_json::Value>> = paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|body| serde_json::from_str(&body).ok())
        .collect();
    let non_empty = |value: Option<&serde_json::Value>| {
        value
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| !s.is_empty())
    };
    let base_url = docs
        .iter()
        .any(|doc| non_empty(doc.get("env").and_then(|env| env.get("ANTHROPIC_BASE_URL"))));
    let helper = docs.iter().any(|doc| non_empty(doc.get("apiKeyHelper")));
    let listed = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if base_url && helper {
        return Check::ok(name, format!("gateway routing and helper set in {listed}"));
    }
    let standalone = standalone_settings_path().map_or_else(
        || "the bridge's claude-code-settings.json".to_owned(),
        |path| path.display().to_string(),
    );
    Check::fail(
        name,
        format!(
            "Claude Code will ask for a login: none of {listed} sets both ANTHROPIC_BASE_URL and \
             apiKeyHelper; run install --host claude-code, or launch with `claude --settings \
             {standalone}`"
        ),
    )
}

pub fn check_config_dir_override(dir: &Path) -> Check {
    Check::warn(
        "claude code config dir",
        format!(
            "{CONFIG_DIR_VAR}={} moves Claude Code's user settings; enrolment writes \
             ~/.claude/settings.json, which this shell's Claude Code does not read",
            dir.display()
        ),
    )
}

// Why: Claude Code prefers an API key or auth token in its environment over
// apiKeyHelper, so either one silently bypasses the gateway credential.
pub fn check_env_credentials(is_set: impl Fn(&str) -> bool) -> Option<Check> {
    let set: Vec<&str> = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"]
        .into_iter()
        .filter(|key| is_set(key))
        .collect();
    (!set.is_empty()).then(|| {
        Check::warn(
            "claude code credentials",
            format!(
                "{} set in this environment, which outranks apiKeyHelper; unset it so Claude \
                 Code uses the gateway credential",
                set.join(" and ")
            ),
        )
    })
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
    let helper_present = root
        .get("apiKeyHelper")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|helper| !helper.is_empty());
    if !helper_present {
        return fail("missing credential helper; run install --host claude-code");
    }
    Check::ok(
        name,
        format!("{}: proxy routing and helper configured", path.display()),
    )
}
