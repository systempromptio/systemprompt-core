//! Governance hooks: the events the harness reports to
//! `/api/public/hooks/track` and the verdict it asks of `/api/public/hooks/
//! govern` before a tool runs, in Claude Code's hook vocabulary.
//!
//! Both calls take a hook token as bearer, the governance plugin's id as
//! `?plugin_id=`, and `x-systemprompt-host`; a track call also carries
//! `x-ingestion-event-id`
//! ([`HookEvent::ingestion_id`])
//! and, once enrolled, the device credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use systemprompt_identifiers::{McpToolName, NativeSessionId, ToolUseId};

/// What every hook event says about where it happened: `session_id` is the v5
/// UUID the instance knows the session by, `native_session_id` the harness's
/// own session id as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookContext {
    pub session_id: NativeSessionId,
    pub native_session_id: NativeSessionId,
    pub cwd: String,
    pub native_host: String,
}

/// What a tool returned, as `PostToolUse` reports it (the whole output; the
/// instance bounds and stores it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub output: String,
    // JSON: tool metadata is free-form and owned by the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// One hook event. The variant is the `hook_event_name`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "hook_event_name")]
pub enum HookEvent {
    SessionStart {
        #[serde(flatten)]
        context: HookContext,
        event_id: String,
    },
    SessionEnd {
        #[serde(flatten)]
        context: HookContext,
        event_id: String,
    },
    Stop {
        #[serde(flatten)]
        context: HookContext,
        event_id: String,
    },
    UserPromptSubmit {
        #[serde(flatten)]
        context: HookContext,
        prompt: String,
        prompt_id: String,
    },
    PreToolUse {
        #[serde(flatten)]
        context: HookContext,
        tool_name: McpToolName,
        // JSON: tool input is the tool's own argument object.
        tool_input: serde_json::Value,
        tool_use_id: ToolUseId,
    },
    PostToolUse {
        #[serde(flatten)]
        context: HookContext,
        tool_name: McpToolName,
        // JSON: tool input is the tool's own argument object.
        tool_input: serde_json::Value,
        tool_use_id: ToolUseId,
        tool_response: ToolResponse,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skill_ref: Option<String>,
    },
}

impl HookEvent {
    #[must_use]
    pub const fn context(&self) -> &HookContext {
        match self {
            Self::SessionStart { context, .. }
            | Self::SessionEnd { context, .. }
            | Self::Stop { context, .. }
            | Self::UserPromptSubmit { context, .. }
            | Self::PreToolUse { context, .. }
            | Self::PostToolUse { context, .. } => context,
        }
    }

    #[must_use]
    pub fn ingestion_id(&self) -> &str {
        match self {
            Self::SessionStart { event_id, .. }
            | Self::SessionEnd { event_id, .. }
            | Self::Stop { event_id, .. } => event_id,
            Self::UserPromptSubmit { prompt_id, .. } => prompt_id,
            Self::PreToolUse { tool_use_id, .. } | Self::PostToolUse { tool_use_id, .. } => {
                tool_use_id.as_str()
            },
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::SessionStart { .. } => "SessionStart",
            Self::SessionEnd { .. } => "SessionEnd",
            Self::Stop { .. } => "Stop",
            Self::UserPromptSubmit { .. } => "UserPromptSubmit",
            Self::PreToolUse { .. } => "PreToolUse",
            Self::PostToolUse { .. } => "PostToolUse",
        }
    }
}

#[must_use]
pub fn skill_ref(owner_plugin: Option<&str>, host: &str, skill_dir: &str) -> String {
    format!("{}:{skill_dir}", owner_plugin.unwrap_or(host))
}

pub const DEFAULT_REFUSAL: &str = "Refused by the instance's governance.";

pub const VERDICT_TIMEOUT_MS: u64 = 3_000;

pub const HOOK_TIMEOUT_MS: u64 = 5_000;

pub const HOOK_TOKEN_REFRESH_SECS: i64 = 300;

/// The `hookSpecificOutput` of a govern answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    #[serde(default)]
    pub permission_decision: Option<String>,
    #[serde(default)]
    pub permission_decision_reason: Option<String>,
}

/// The answer of `/api/public/hooks/govern`. The hook answers 200 with a
/// decision for every verdict it reaches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernAnswer {
    pub hook_specific_output: HookSpecificOutput,
}

impl GovernAnswer {
    #[must_use]
    pub fn refusal(&self) -> Option<&str> {
        let output = &self.hook_specific_output;
        (output.permission_decision.as_deref() == Some("deny")).then(|| {
            output
                .permission_decision_reason
                .as_deref()
                .unwrap_or(DEFAULT_REFUSAL)
        })
    }
}

#[must_use]
pub fn session_metadata_user_id(session: &Uuid) -> String {
    serde_json::json!({ "session_id": session.hyphenated().to_string() }).to_string()
}

// JSON: a model request body of any provider shape, stamped with the session
// metadata.
pub fn stamp_session_metadata(body: &mut serde_json::Value, session: &Uuid) -> bool {
    let Some(object) = body.as_object_mut() else {
        return false;
    };
    let metadata = object
        .entry("metadata")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(metadata) = metadata.as_object_mut() else {
        return false;
    };
    if metadata.contains_key("user_id") {
        return false;
    }
    metadata.insert(
        "user_id".to_owned(),
        serde_json::Value::String(session_metadata_user_id(session)),
    );
    true
}
