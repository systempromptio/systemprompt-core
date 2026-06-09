//! Declarative system-prompt override rules.
//!
//! A [`SystemPromptRule`] optionally matches on the resolved upstream
//! `provider` and the requested `model_pattern` (the same exact / prefix `foo*`
//! / suffix `*foo` / catch-all `*` grammar as [`super::route::GatewayRoute`]),
//! and applies a [`OverrideRuleAction`] to the inbound request's system prompt
//! before it is forwarded upstream: `replace` substitutes a fixed `prompt`,
//! `strip` removes the prompt entirely. Rules are pure data; resolution and
//! application live in the gateway dispatch layer.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ProviderId;

use super::error::{GatewayProfileError, GatewayResult};
use super::route::match_pattern;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverrideRuleAction {
    Replace,
    Strip,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SystemPromptRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_pattern: Option<String>,
    pub action: OverrideRuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl SystemPromptRule {
    #[must_use]
    pub fn matches(&self, provider: &ProviderId, model: &str) -> bool {
        let provider_ok = self
            .provider
            .as_ref()
            .is_none_or(|p| p.as_str() == provider.as_str());
        let model_ok = self
            .model_pattern
            .as_deref()
            .is_none_or(|pat| match_pattern(pat, model));
        provider_ok && model_ok
    }

    pub const fn validate(&self) -> GatewayResult<()> {
        match self.action {
            OverrideRuleAction::Replace if self.prompt.is_none() => {
                Err(GatewayProfileError::OverrideReplaceMissingPrompt)
            },
            OverrideRuleAction::Strip if self.prompt.is_some() => {
                Err(GatewayProfileError::OverrideStripWithPrompt)
            },
            _ => Ok(()),
        }
    }
}
