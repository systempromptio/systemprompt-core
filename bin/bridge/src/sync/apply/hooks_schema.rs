//! Typed wire schema for the Cowork `hooks.json` file emitted per-plugin.
//!
//! `Http` entries are gateway-issued govern/track proxies that present the
//! static loopback secret as `Authorization`; `Command` entries are
//! user-defined hooks from `services/hooks/` YAML. `allowedEnvVars` is empty —
//! Cowork's agent VM does not reliably propagate plugin env vars.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HooksFile {
    pub hooks: BTreeMap<String, Vec<HookMatcher>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum HookEntry {
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
    pub(crate) fn govern(url: String, authorization: &str) -> Self {
        Self::Http {
            url,
            headers: bearer_header(authorization),
            allowed_env_vars: vec![],
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: None,
            event: None,
        }
    }

    pub(crate) fn track(url: String, authorization: &str, event: &str) -> Self {
        Self::Http {
            url,
            headers: bearer_header(authorization),
            allowed_env_vars: vec![],
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: Some(true),
            event: Some(event.to_owned()),
        }
    }

    pub(crate) fn user_command(command: String, event: &str, is_async: bool) -> Self {
        Self::Command {
            command,
            timeout: DEFAULT_TIMEOUT_SECS,
            r#async: if is_async { Some(true) } else { None },
            event: event.to_owned(),
        }
    }
}

impl HookMatcher {
    pub(crate) fn wildcard(entry: HookEntry) -> Self {
        Self {
            matcher: WILDCARD_MATCHER.to_owned(),
            hooks: vec![entry],
        }
    }
}

impl HooksFile {
    pub(crate) fn new(govern_url: String, track_url: &str, authorization: &str) -> Self {
        let mut hooks: BTreeMap<String, Vec<HookMatcher>> = BTreeMap::new();
        hooks.insert(
            "PreToolUse".to_owned(),
            vec![HookMatcher::wildcard(HookEntry::govern(
                govern_url,
                authorization,
            ))],
        );
        for event in TRACK_EVENTS {
            hooks.insert(
                (*event).to_owned(),
                vec![HookMatcher::wildcard(HookEntry::track(
                    track_url.to_owned(),
                    authorization,
                    event,
                ))],
            );
        }
        Self { hooks }
    }

    pub(crate) fn append_user_hook(&mut self, event: String, matcher: String, entry: HookEntry) {
        let bucket = self.hooks.entry(event).or_default();
        let m = if matcher.is_empty() {
            WILDCARD_MATCHER.to_owned()
        } else {
            matcher
        };
        bucket.push(HookMatcher {
            matcher: m,
            hooks: vec![entry],
        });
    }
}

fn bearer_header(authorization: &str) -> BTreeMap<String, String> {
    let mut h = BTreeMap::new();
    h.insert("Authorization".to_owned(), authorization.to_owned());
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
