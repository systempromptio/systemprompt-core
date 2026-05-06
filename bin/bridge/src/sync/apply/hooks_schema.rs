//! Typed wire schema for the Cowork `hooks.json` file emitted per-plugin.
//!
//! The bridge writes one of these into each synced plugin's `hooks/` directory.
//! Cowork reads it at hook-fire time, substitutes `$SYSTEMPROMPT_PLUGIN_TOKEN`
//! from the sibling `.env.plugin`, and dispatches to the bridge gateway's
//! public hook endpoints. The shape is camelCase on the wire; field names here
//! are snake_case with `#[serde(rename_all = "camelCase")]`.
//!
//! Wired into the call site in Stage 3B; allow-dead-code until then.

#![allow(dead_code)]

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
#[serde(rename_all = "camelCase")]
pub struct HookEntry {
    #[serde(rename = "type")]
    pub kind: HookKind,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub allowed_env_vars: Vec<String>,
    pub timeout: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#async: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookKind {
    Http,
}

impl HookEntry {
    pub fn govern(url: String, token_env_var: &str) -> Self {
        Self {
            kind: HookKind::Http,
            url,
            headers: bearer_header(token_env_var),
            allowed_env_vars: vec![token_env_var.to_string()],
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: None,
            event: None,
        }
    }

    pub fn track(url: String, token_env_var: &str, event: &str) -> Self {
        Self {
            kind: HookKind::Http,
            url,
            headers: bearer_header(token_env_var),
            allowed_env_vars: vec![token_env_var.to_string()],
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: Some(true),
            event: Some(event.to_string()),
        }
    }
}

impl HookMatcher {
    pub fn wildcard(entry: HookEntry) -> Self {
        Self {
            matcher: WILDCARD_MATCHER.to_string(),
            hooks: vec![entry],
        }
    }
}

impl HooksFile {
    pub fn new(govern_url: String, track_url: &str, token_env_var: &str) -> Self {
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
