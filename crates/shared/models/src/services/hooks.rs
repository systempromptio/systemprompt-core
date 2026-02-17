use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct HookEventsConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_tool_use: Vec<HookMatcher>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_tool_use: Vec<HookMatcher>,
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
            && self.session_start.is_empty()
            && self.session_end.is_empty()
            && self.user_prompt_submit.is_empty()
            && self.notification.is_empty()
            && self.stop.is_empty()
            && self.subagent_start.is_empty()
            && self.subagent_stop.is_empty()
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let all_matchers: Vec<&HookMatcher> = self
            .pre_tool_use
            .iter()
            .chain(&self.post_tool_use)
            .chain(&self.session_start)
            .chain(&self.session_end)
            .chain(&self.user_prompt_submit)
            .chain(&self.notification)
            .chain(&self.stop)
            .chain(&self.subagent_start)
            .chain(&self.subagent_stop)
            .collect();

        for matcher in all_matchers {
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

        Ok(())
    }
}
