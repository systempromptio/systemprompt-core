//! Whether the Claude Code CLI's settings route its inference to the gateway.
//!
//! Claude Code reads its managed-policy file and its user settings file, the
//! policy file winning; it reaches the gateway only when the effective
//! `env.ANTHROPIC_BASE_URL` and `apiKeyHelper` are the values this bridge
//! would write now. A helper naming a bridge executable that has since moved
//! (a reinstall elsewhere) is set but dead, and Claude Code falls back to its
//! login prompt. The doctor's routing check, the GUI's agent verdict and the
//! sync's repair all ask this one question, so they cannot disagree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    pub helper: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    Routed,
    Stale(StaleKey),
    Unrouted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleKey {
    ApiKeyHelper,
    BaseUrl,
}

impl StaleKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiKeyHelper => "apiKeyHelper",
            Self::BaseUrl => "ANTHROPIC_BASE_URL",
        }
    }
}

impl Routing {
    #[must_use]
    pub const fn is_routed(self) -> bool {
        matches!(self, Self::Routed)
    }
}

#[must_use]
pub fn read_paths(config_dir: Option<&Path>) -> Vec<PathBuf> {
    let policy = crate::config::paths::claude_code_policy_dir().join("managed-settings.json");
    let user = config_dir.map_or_else(crate::config::paths::claude_cli_settings_path, |dir| {
        Some(dir.join("settings.json"))
    });
    std::iter::once(policy).chain(user).collect()
}

#[must_use]
pub fn effective_read_paths() -> Vec<PathBuf> {
    read_paths(crate::config::paths::claude_config_dir_override().as_deref())
}

#[must_use]
pub fn expected(base_url: Option<&str>) -> Option<Expected> {
    Some(Expected {
        helper: crate::install::mdm::claude_code_settings::current_helper_command()?,
        base_url: base_url.map(str::to_owned),
    })
}

#[must_use]
pub fn routing(paths: &[PathBuf], expected: &Expected) -> Routing {
    let docs: Vec<serde_json::Map<String, serde_json::Value>> = paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|body| serde_json::from_str(&body).ok())
        .collect();
    let effective = |pick: &dyn Fn(
        &serde_json::Map<String, serde_json::Value>,
    ) -> Option<&serde_json::Value>| {
        docs.iter()
            .filter_map(|doc| pick(doc).and_then(serde_json::Value::as_str))
            .find(|s| !s.is_empty())
    };
    let base_url = effective(&|doc| doc.get("env").and_then(|env| env.get("ANTHROPIC_BASE_URL")));
    let helper = effective(&|doc| doc.get("apiKeyHelper"));
    let (Some(base_url), Some(helper)) = (base_url, helper) else {
        return Routing::Unrouted;
    };
    if helper != expected.helper {
        return Routing::Stale(StaleKey::ApiKeyHelper);
    }
    if expected
        .base_url
        .as_deref()
        .is_some_and(|want| want != base_url)
    {
        return Routing::Stale(StaleKey::BaseUrl);
    }
    Routing::Routed
}

#[must_use]
pub fn current_routing(base_url: Option<&str>) -> Routing {
    expected(base_url).map_or(Routing::Unrouted, |want| {
        routing(&effective_read_paths(), &want)
    })
}

#[must_use]
pub fn is_routed() -> bool {
    current_routing(None).is_routed()
}
