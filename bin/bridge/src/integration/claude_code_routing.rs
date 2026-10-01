//! Whether the Claude Code CLI's settings route its inference to the gateway.
//!
//! Claude Code reads its managed-policy file and its user settings file; it
//! reaches the gateway only when, between them, `env.ANTHROPIC_BASE_URL` and
//! `apiKeyHelper` are both set. The doctor's routing check and the GUI's agent
//! verdict both ask this one question, so they cannot disagree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

pub const CONFIG_DIR_VAR: &str = "CLAUDE_CONFIG_DIR";

#[must_use]
pub fn read_paths(config_dir: Option<&Path>) -> Vec<PathBuf> {
    let policy = crate::config::paths::claude_code_policy_dir().join("managed-settings.json");
    let user = config_dir.map_or_else(crate::config::paths::claude_cli_settings_path, |dir| {
        Some(dir.join("settings.json"))
    });
    std::iter::once(policy).chain(user).collect()
}

#[must_use]
pub fn config_dir_override() -> Option<PathBuf> {
    std::env::var_os(CONFIG_DIR_VAR).map(PathBuf::from)
}

#[must_use]
pub fn effective_read_paths() -> Vec<PathBuf> {
    read_paths(config_dir_override().as_deref())
}

#[must_use]
pub fn routes_through_gateway(paths: &[PathBuf]) -> bool {
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
    base_url && helper
}

#[must_use]
pub fn is_routed() -> bool {
    routes_through_gateway(&effective_read_paths())
}
