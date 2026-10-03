//! Hook configuration: lifecycle events, matchers, and actions.
//!
//! [`HookEventsConfig`] groups the [`HookMatcher`]/[`HookAction`] bindings per
//! event and validates them via [`HookEventsConfig::validate`].
//! [`DiskHookConfig`] is the per-hook on-disk descriptor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Deserializer, Serialize};
use systemprompt_identifiers::HookId;

use systemprompt_models::errors::ServicesValidationError;
use systemprompt_models::hooks::{HookCategory, HookEvent};

pub const HOOK_CONFIG_FILENAME: &str = "config.yaml";

const fn default_true() -> bool {
    true
}

fn default_version() -> String {
    "1.0.0".to_owned()
}

fn default_matcher() -> String {
    "*".to_owned()
}

fn blank_hook_id_as_none<'de, D>(deserializer: D) -> Result<Option<HookId>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .filter(|raw| !raw.trim().is_empty())
        .map(HookId::try_new)
        .transpose()
        .map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskHookConfig {
    #[serde(default, deserialize_with = "blank_hook_id_as_none")]
    pub id: Option<HookId>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub event: HookEvent,
    #[serde(default = "default_matcher")]
    pub matcher: String,
    #[serde(default)]
    pub command: String,
    #[serde(default, rename = "async")]
    pub is_async: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(default)]
    pub category: HookCategory,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub visible_to: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct HookEventsConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_tool_use: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_tool_use: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_tool_use_failure: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_start: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub session_end: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_prompt_submit: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notification: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagent_start: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subagent_stop: Vec<HookMatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HookAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookAction {
    #[serde(rename = "type")]
    pub hook_type: HookType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, rename = "async")]
    pub r#async: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "statusMessage")]
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookType {
    Command,
    Prompt,
    Agent,
}

impl HookEventsConfig {
    pub const fn is_empty(&self) -> bool {
        self.pre_tool_use.is_empty()
            && self.post_tool_use.is_empty()
            && self.post_tool_use_failure.is_empty()
            && self.session_start.is_empty()
            && self.session_end.is_empty()
            && self.user_prompt_submit.is_empty()
            && self.notification.is_empty()
            && self.stop.is_empty()
            && self.subagent_start.is_empty()
            && self.subagent_stop.is_empty()
    }

    pub fn matchers_for_event(&self, event: HookEvent) -> &[HookMatcher] {
        match event {
            HookEvent::PreToolUse => &self.pre_tool_use,
            HookEvent::PostToolUse => &self.post_tool_use,
            HookEvent::PostToolUseFailure => &self.post_tool_use_failure,
            HookEvent::SessionStart => &self.session_start,
            HookEvent::SessionEnd => &self.session_end,
            HookEvent::UserPromptSubmit => &self.user_prompt_submit,
            HookEvent::Notification => &self.notification,
            HookEvent::Stop => &self.stop,
            HookEvent::SubagentStart => &self.subagent_start,
            HookEvent::SubagentStop => &self.subagent_stop,
        }
    }

    pub fn validate(&self) -> Result<(), ServicesValidationError> {
        for event in HookEvent::ALL_VARIANTS {
            for matcher in self.matchers_for_event(*event) {
                for action in &matcher.hooks {
                    match action.hook_type {
                        HookType::Command => {
                            if action.command.is_none() {
                                return Err(ServicesValidationError::required(format!(
                                    "Hook matcher '{}': command hook requires a 'command' field",
                                    matcher.matcher
                                )));
                            }
                        },
                        HookType::Prompt => {
                            if action.prompt.is_none() {
                                return Err(ServicesValidationError::required(format!(
                                    "Hook matcher '{}': prompt hook requires a 'prompt' field",
                                    matcher.matcher
                                )));
                            }
                        },
                        HookType::Agent => {},
                    }
                }
            }
        }

        Ok(())
    }
}
