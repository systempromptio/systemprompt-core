//! Typed wire schema for the Cowork `hooks.json` file emitted per-plugin.
//!
//! The bridge writes one of these into each synced plugin's `hooks/` directory.
//! Cowork reads it at hook-fire time, substitutes `$SYSTEMPROMPT_PLUGIN_TOKEN`
//! from the sibling `.env.plugin`, and dispatches to the bridge gateway's
//! public hook endpoints. The shape is camelCase on the wire; field names here
//! are `snake_case` with `#[serde(rename_all = "camelCase")]`.
//!
//! Two variants share the file: `Http` entries are gateway-issued govern/track
//! proxies; `Command` entries are user-defined hooks sourced from
//! `services/hooks/` YAML in the manifest. The `type` discriminant on
//! `HookEntry` keeps the existing HTTP wire shape byte-identical and adds a
//! sibling shape for command hooks.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksFile {
    pub hooks: BTreeMap<String, Vec<HookMatcher>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HookEntry {
    Http {
        url: String,
        headers: BTreeMap<String, String>,
        #[serde(rename = "allowedEnvVars")]
        allowed_env_vars: Vec<String>,
        timeout: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r#async: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        event: Option<String>,
    },
    Command {
        command: String,
        timeout: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r#async: Option<bool>,
        event: String,
    },
}

impl HookEntry {
    pub(crate) fn govern(url: String, token_env_var: &str) -> Self {
        Self::Http {
            url,
            headers: bearer_header(token_env_var),
            allowed_env_vars: vec![token_env_var.to_string()],
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: None,
            event: None,
        }
    }

    pub(crate) fn track(url: String, token_env_var: &str, event: &str) -> Self {
        Self::Http {
            url,
            headers: bearer_header(token_env_var),
            allowed_env_vars: vec![token_env_var.to_string()],
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: Some(true),
            event: Some(event.to_string()),
        }
    }

    pub(crate) fn user_command(command: String, event: &str, is_async: bool) -> Self {
        Self::Command {
            command,
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: if is_async { Some(true) } else { None },
            event: event.to_string(),
        }
    }
}

impl HookMatcher {
    pub(crate) fn wildcard(entry: HookEntry) -> Self {
        Self {
            matcher: WILDCARD_MATCHER.to_string(),
            hooks: vec![entry],
        }
    }
}

impl HooksFile {
    pub(crate) fn new(govern_url: String, track_url: &str, token_env_var: &str) -> Self {
        let mut hooks: BTreeMap<String, Vec<HookMatcher>> = BTreeMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![HookMatcher::wildcard(HookEntry::govern(
                govern_url,
                token_env_var,
            ))],
        );
        for event in TRACK_EVENTS {
            hooks.insert(
                (*event).to_string(),
                vec![HookMatcher::wildcard(HookEntry::track(
                    track_url.to_string(),
                    token_env_var,
                    event,
                ))],
            );
        }
        Self { hooks }
    }

    pub(crate) fn append_user_hook(&mut self, event: String, matcher: String, entry: HookEntry) {
        let bucket = self.hooks.entry(event).or_default();
        let m = if matcher.is_empty() {
            WILDCARD_MATCHER.to_string()
        } else {
            matcher
        };
        bucket.push(HookMatcher {
            matcher: m,
            hooks: vec![entry],
        });
    }
}

fn bearer_header(token_env_var: &str) -> BTreeMap<String, String> {
    let mut h = BTreeMap::new();
    h.insert(
        "Authorization".to_string(),
        format!("Bearer ${token_env_var}"),
    );
    h
}

const WILDCARD_MATCHER: &str = "*";
const DEFAULT_TIMEOUT_SECS: u32 = 10;

const TRACK_EVENTS: &[&str] = &[
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
