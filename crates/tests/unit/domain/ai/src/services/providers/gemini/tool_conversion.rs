//! Tests for Gemini tool conversion functions.

use systemprompt_ai::services::providers::gemini::tool_conversion::build_thinking_config;

mod build_thinking_config_tests {
    use super::*;

    #[test]
    fn returns_config_for_2_5_models() {
        let config = build_thinking_config("gemini-2.5-flash");
        let config = config.expect("config should be present");
        config.thinking_budget.expect("thinking_budget should be set");
        assert_eq!(config.include_thoughts, Some(false));
    }

    #[test]
    fn returns_config_for_2_5_pro_model() {
        let config = build_thinking_config("gemini-2.5-pro");
        config.as_ref().expect("config should be present");
    }

    #[test]
    fn returns_config_for_2_5_flash_lite() {
        let config = build_thinking_config("gemini-2.5-flash-lite");
        config.as_ref().expect("config should be present");
    }

    #[test]
    fn returns_none_for_1_5_models() {
        let config = build_thinking_config("gemini-1.5-flash");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_1_5_pro() {
        let config = build_thinking_config("gemini-1.5-pro");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_other_models() {
        let config = build_thinking_config("gemini-pro");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_non_gemini_models() {
        let config = build_thinking_config("gpt-4");
        assert!(config.is_none());
    }

    #[test]
    fn model_name_case_sensitive() {
        // The check uses contains("2.5") which is case-sensitive
        // but "2.5" itself doesn't have case, so uppercase model names also match
        let config = build_thinking_config("GEMINI-2.5-FLASH");
        // Since "2.5" is found in the string regardless of case of letters around it
        config.as_ref().expect("config should be present");
    }
}
