use crate::models::providers::anthropic::AnthropicThinking;

pub mod tokens {
    pub const THINKING_BUDGET: u32 = 10240;
}

pub fn build_thinking_config(model: &str) -> Option<AnthropicThinking> {
    if supports_extended_thinking(model) {
        Some(AnthropicThinking {
            thinking_type: "enabled".to_string(),
            budget_tokens: tokens::THINKING_BUDGET,
        })
    } else {
        None
    }
}

pub fn supports_extended_thinking(model: &str) -> bool {
    model.contains("claude-3-5") || model.contains("claude-3.5")
}
