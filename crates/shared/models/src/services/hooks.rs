use std::fmt;
use std::str::FromStr;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::HookId;

pub const HOOK_CONFIG_FILENAME: &str = "config.yaml";

const fn default_true() -> bool {
    true
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_matcher() -> String {
    "*".to_string()
}

fn default_hook_id() -> HookId {
    HookId::new("")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    Notification,
    Stop,
    SubagentStart,
    SubagentStop,
}

impl HookEvent {
    pub const ALL_VARIANTS: &'static [Self] = &[
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::SessionStart,
        Self::SessionEnd,
        Self::UserPromptSubmit,
        Self::Notification,
        Self::Stop,
        Self::SubagentStart,
        Self::SubagentStop,
    ];

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Notification => "Notification",
            Self::Stop => "Stop",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
        }
    }
}

impl fmt::Display for HookEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HookEvent {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "PreToolUse" => Ok(Self::PreToolUse),
            "PostToolUse" => Ok(Self::PostToolUse),
            "PostToolUseFailure" => Ok(Self::PostToolUseFailure),
            "SessionStart" => Ok(Self::SessionStart),
            "SessionEnd" => Ok(Self::SessionEnd),
            "UserPromptSubmit" => Ok(Self::UserPromptSubmit),
            "Notification" => Ok(Self::Notification),
            "Stop" => Ok(Self::Stop),
            "SubagentStart" => Ok(Self::SubagentStart),
            "SubagentStop" => Ok(Self::SubagentStop),
            _ => Err(anyhow!("Invalid hook event: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookCategory {
    System,
    #[default]
    Custom,
}

impl HookCategory {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Custom => "custom",
        }
    }
}

impl fmt::Display for HookCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HookCategory {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "system" => Ok(Self::System),
            "custom" => Ok(Self::Custom),
            _ => Err(anyhow!("Invalid hook category: {s}")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskHookConfig {
    #[serde(default = "default_hook_id")]
    pub id: HookId,
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
    pub fn is_empty(&self) -> bool {
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

    pub fn validate(&self) -> anyhow::Result<()> {
        for event in HookEvent::ALL_VARIANTS {
            for matcher in self.matchers_for_event(*event) {
                for action in &matcher.hooks {
                    match action.hook_type {
                        HookType::Command => {
                            if action.command.is_none() {
                                anyhow::bail!(
                                    "Hook matcher '{}': command hook requires a 'command' field",
                                    matcher.matcher
                                );
                            }
                        },
                        HookType::Prompt => {
                            if action.prompt.is_none() {
                                anyhow::bail!(
                                    "Hook matcher '{}': prompt hook requires a 'prompt' field",
                                    matcher.matcher
                                );
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
