use systemprompt_core_ai::services::providers::anthropic::thinking::{
    build_thinking_config, supports_extended_thinking,
};

mod build_thinking_config_tests {
    use super::*;

    #[test]
    fn returns_config_for_claude_3_5_sonnet() {
        let config = build_thinking_config("claude-3-5-sonnet-20241022");
        assert!(config.is_some());
        let config = config.expect("config should exist");
        assert_eq!(config.thinking_type, "enabled");
        assert_eq!(config.budget_tokens, 10240);
    }

    #[test]
    fn returns_config_for_claude_3_5_haiku() {
        let config = build_thinking_config("claude-3-5-haiku-20241022");
        assert!(config.is_some());
    }

    #[test]
    fn returns_config_for_claude_3_5_with_dot() {
        let config = build_thinking_config("claude-3.5-sonnet");
        assert!(config.is_some());
    }

    #[test]
    fn returns_none_for_claude_3_opus() {
        let config = build_thinking_config("claude-3-opus-20240229");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_claude_3_sonnet() {
        let config = build_thinking_config("claude-3-sonnet-20240229");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_claude_3_haiku() {
        let config = build_thinking_config("claude-3-haiku-20240307");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_non_anthropic_models() {
        let config = build_thinking_config("gpt-4");
        assert!(config.is_none());
    }

    #[test]
    fn thinking_type_is_enabled() {
        let config = build_thinking_config("claude-3-5-sonnet-20241022").expect("should have config");
        assert_eq!(config.thinking_type, "enabled");
    }

    #[test]
    fn budget_tokens_has_correct_value() {
        let config = build_thinking_config("claude-3-5-sonnet-20241022").expect("should have config");
        assert_eq!(config.budget_tokens, 10240);
    }
}

mod supports_extended_thinking_tests {
    use super::*;

    #[test]
    fn returns_true_for_claude_3_5_models() {
        assert!(supports_extended_thinking("claude-3-5-sonnet-20241022"));
        assert!(supports_extended_thinking("claude-3-5-haiku-20241022"));
    }

    #[test]
    fn returns_true_for_claude_3_dot_5_models() {
        assert!(supports_extended_thinking("claude-3.5-sonnet"));
    }

    #[test]
    fn returns_false_for_claude_3_models() {
        assert!(!supports_extended_thinking("claude-3-opus-20240229"));
        assert!(!supports_extended_thinking("claude-3-sonnet-20240229"));
        assert!(!supports_extended_thinking("claude-3-haiku-20240307"));
    }

    #[test]
    fn returns_false_for_other_providers() {
        assert!(!supports_extended_thinking("gpt-4"));
        assert!(!supports_extended_thinking("gemini-2.5-flash"));
    }
}
