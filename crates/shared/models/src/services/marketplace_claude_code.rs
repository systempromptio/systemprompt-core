//! Claude Code client settings a marketplace declares for its subscribers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::errors::ConfigValidationError;

/// Claude Code settings the bridge writes into the client for a marketplace.
///
/// `skill_listing_budget_chars` becomes `env.SLASH_COMMAND_TOOL_CHAR_BUDGET`:
/// the characters Claude Code may spend listing skills before it truncates
/// their descriptions. With several marketplaces the largest value wins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeCodeMarketplaceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_listing_budget_chars: Option<u32>,
}

impl ClaudeCodeMarketplaceConfig {
    pub(super) fn validate(&self, key: &str) -> Result<(), ConfigValidationError> {
        if self.skill_listing_budget_chars == Some(0) {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': claude_code.skill_listing_budget_chars must be greater \
                 than zero"
            )));
        }
        Ok(())
    }
}
