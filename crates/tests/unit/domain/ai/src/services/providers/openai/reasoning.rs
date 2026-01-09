use systemprompt_core_ai::models::providers::openai::OpenAiReasoningEffort;
use systemprompt_core_ai::services::providers::openai::reasoning::{
    build_reasoning_config, is_reasoning_model,
};

mod build_reasoning_config_tests {
    use super::*;

    #[test]
    fn returns_config_for_o1_model() {
        let config = build_reasoning_config("o1");
        assert!(config.is_some());
        let config = config.expect("config should exist");
        assert!(matches!(config, OpenAiReasoningEffort::Medium));
    }

    #[test]
    fn returns_config_for_o1_mini() {
        let config = build_reasoning_config("o1-mini");
        assert!(config.is_some());
    }

    #[test]
    fn returns_config_for_o1_preview() {
        let config = build_reasoning_config("o1-preview");
        assert!(config.is_some());
    }

    #[test]
    fn returns_config_for_o3_model() {
        let config = build_reasoning_config("o3");
        assert!(config.is_some());
    }

    #[test]
    fn returns_config_for_o3_mini() {
        let config = build_reasoning_config("o3-mini");
        assert!(config.is_some());
    }

    #[test]
    fn returns_none_for_gpt4() {
        let config = build_reasoning_config("gpt-4");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_gpt4o() {
        let config = build_reasoning_config("gpt-4o");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_gpt4o_mini() {
        let config = build_reasoning_config("gpt-4o-mini");
        assert!(config.is_none());
    }

    #[test]
    fn returns_none_for_gpt35_turbo() {
        let config = build_reasoning_config("gpt-3.5-turbo");
        assert!(config.is_none());
    }

    #[test]
    fn default_reasoning_effort_is_medium() {
        let config = build_reasoning_config("o1").expect("should have config");
        assert!(matches!(config, OpenAiReasoningEffort::Medium));
    }
}

mod is_reasoning_model_tests {
    use super::*;

    #[test]
    fn o1_is_reasoning_model() {
        assert!(is_reasoning_model("o1"));
        assert!(is_reasoning_model("o1-mini"));
        assert!(is_reasoning_model("o1-preview"));
    }

    #[test]
    fn o3_is_reasoning_model() {
        assert!(is_reasoning_model("o3"));
        assert!(is_reasoning_model("o3-mini"));
    }

    #[test]
    fn gpt4_is_not_reasoning_model() {
        assert!(!is_reasoning_model("gpt-4"));
        assert!(!is_reasoning_model("gpt-4-turbo"));
        assert!(!is_reasoning_model("gpt-4o"));
        assert!(!is_reasoning_model("gpt-4o-mini"));
    }

    #[test]
    fn gpt35_is_not_reasoning_model() {
        assert!(!is_reasoning_model("gpt-3.5-turbo"));
    }

    #[test]
    fn other_providers_are_not_reasoning_models() {
        assert!(!is_reasoning_model("claude-3-5-sonnet"));
        assert!(!is_reasoning_model("gemini-2.5-flash"));
    }
}
