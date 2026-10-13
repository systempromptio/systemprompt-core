//! The typed wire schema of the run-local governance plugin Claude Code
//! loads: `.claude-plugin/plugin.json` and `hooks/hooks.json`.
//!
//! Same shape as the bridge's `host_sync/hooks_schema.rs`: an http
//! `PreToolUse` hook to `govern` and async http track hooks per lifecycle
//! event, each carrying the hook token as a bearer header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const HOOK_TIMEOUT_SECS: u32 = 10;

pub const TRACK_EVENTS: &[&str] = &[
    "PostToolUse",
    "PostToolUseFailure",
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "Notification",
    "TaskCompleted",
    "TeammateIdle",
    "PermissionRequest",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HooksFile {
    pub hooks: BTreeMap<String, Vec<HookMatcher>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HttpHook>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpHook {
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    #[serde(rename = "allowedEnvVars")]
    pub allowed_env_vars: Vec<String>,
    pub timeout: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#async: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

impl HttpHook {
    #[must_use]
    pub fn govern(url: String, token: &str) -> Self {
        Self::new(url, token, None)
    }

    #[must_use]
    pub fn track(url: String, token: &str, event: &str) -> Self {
        Self::new(url, token, Some(event))
    }

    fn new(url: String, token: &str, event: Option<&str>) -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".to_owned(), format!("Bearer {token}"));
        Self {
            kind: "http".to_owned(),
            url,
            headers,
            allowed_env_vars: Vec::new(),
            timeout: HOOK_TIMEOUT_SECS,
            r#async: event.map(|_| true),
            event: event.map(str::to_owned),
        }
    }
}

impl HookMatcher {
    #[must_use]
    pub fn any(hook: HttpHook) -> Self {
        Self {
            matcher: "*".to_owned(),
            hooks: vec![hook],
        }
    }
}
