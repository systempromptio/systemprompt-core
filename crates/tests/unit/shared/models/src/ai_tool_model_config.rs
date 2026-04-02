use systemprompt_models::ai::ToolModelConfig;

mod tool_model_config_construction_tests {
    use super::*;

    #[test]
    fn new_sets_provider_and_model() {
        let config = ToolModelConfig::new("anthropic", "claude-3-sonnet");

        assert_eq!(config.provider, Some("anthropic".to_string()));
        assert_eq!(config.model, Some("claude-3-sonnet".to_string()));
        assert!(config.max_output_tokens.is_none());
    }

    #[test]
    fn new_accepts_string_types() {
        let config = ToolModelConfig::new(String::from("openai"), String::from("gpt-4"));

        assert_eq!(config.provider, Some("openai".to_string()));
        assert_eq!(config.model, Some("gpt-4".to_string()));
    }

    #[test]
    fn with_max_output_tokens_sets_value() {
        let config = ToolModelConfig::new("provider", "model").with_max_output_tokens(4096);

        assert_eq!(config.max_output_tokens, Some(4096));
    }

    #[test]
    fn default_has_all_none() {
        let config = ToolModelConfig::default();

        assert!(config.provider.is_none());
        assert!(config.model.is_none());
        assert!(config.max_output_tokens.is_none());
    }
}

mod tool_model_config_is_empty_tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let config = ToolModelConfig::default();
        assert!(config.is_empty());
    }

    #[test]
    fn with_provider_is_not_empty() {
        let config = ToolModelConfig {
            provider: Some("anthropic".to_string()),
            model: None,
            max_output_tokens: None,
        };
        assert!(!config.is_empty());
    }

    #[test]
    fn with_model_is_not_empty() {
        let config = ToolModelConfig {
            provider: None,
            model: Some("gpt-4".to_string()),
            max_output_tokens: None,
        };
        assert!(!config.is_empty());
    }

    #[test]
    fn with_max_output_tokens_is_not_empty() {
        let config = ToolModelConfig {
            provider: None,
            model: None,
            max_output_tokens: Some(1024),
        };
        assert!(!config.is_empty());
    }

    #[test]
    fn fully_populated_is_not_empty() {
        let config = ToolModelConfig::new("provider", "model").with_max_output_tokens(2048);
        assert!(!config.is_empty());
    }
}

mod tool_model_config_merge_tests {
    use super::*;

    #[test]
    fn merge_with_empty_keeps_original() {
        let base = ToolModelConfig::new("anthropic", "claude-3");
        let empty = ToolModelConfig::default();

        let merged = base.merge_with(&empty);

        assert_eq!(merged.provider, Some("anthropic".to_string()));
        assert_eq!(merged.model, Some("claude-3".to_string()));
    }

    #[test]
    fn merge_with_overrides_all_fields() {
        let base = ToolModelConfig::new("anthropic", "claude-3").with_max_output_tokens(1024);
        let override_config = ToolModelConfig::new("openai", "gpt-4").with_max_output_tokens(4096);

        let merged = base.merge_with(&override_config);

        assert_eq!(merged.provider, Some("openai".to_string()));
        assert_eq!(merged.model, Some("gpt-4".to_string()));
        assert_eq!(merged.max_output_tokens, Some(4096));
    }

    #[test]
    fn merge_with_partial_override_keeps_base_for_missing() {
        let base = ToolModelConfig::new("anthropic", "claude-3").with_max_output_tokens(1024);
        let partial = ToolModelConfig {
            provider: None,
            model: Some("claude-4".to_string()),
            max_output_tokens: None,
        };

        let merged = base.merge_with(&partial);

        assert_eq!(merged.provider, Some("anthropic".to_string()));
        assert_eq!(merged.model, Some("claude-4".to_string()));
        assert_eq!(merged.max_output_tokens, Some(1024));
    }

    #[test]
    fn merge_empty_with_empty_stays_empty() {
        let empty1 = ToolModelConfig::default();
        let empty2 = ToolModelConfig::default();

        let merged = empty1.merge_with(&empty2);

        assert!(merged.is_empty());
    }

    #[test]
    fn merge_empty_base_with_full_override() {
        let base = ToolModelConfig::default();
        let full = ToolModelConfig::new("provider", "model").with_max_output_tokens(2048);

        let merged = base.merge_with(&full);

        assert_eq!(merged.provider, Some("provider".to_string()));
        assert_eq!(merged.model, Some("model".to_string()));
        assert_eq!(merged.max_output_tokens, Some(2048));
    }
}

mod tool_model_config_serialization_tests {
    use super::*;

    #[test]
    fn serializes_full_config() {
        let config = ToolModelConfig::new("anthropic", "claude-3").with_max_output_tokens(4096);

        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains("anthropic"));
        assert!(json.contains("claude-3"));
        assert!(json.contains("4096"));
    }

    #[test]
    fn skips_none_fields_in_serialization() {
        let config = ToolModelConfig {
            provider: Some("anthropic".to_string()),
            model: None,
            max_output_tokens: None,
        };

        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains("anthropic"));
        assert!(!json.contains("model"));
        assert!(!json.contains("max_output_tokens"));
    }

    #[test]
    fn deserializes_full_config() {
        let json = r#"{"provider":"openai","model":"gpt-4","max_output_tokens":8192}"#;

        let config: ToolModelConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.provider, Some("openai".to_string()));
        assert_eq!(config.model, Some("gpt-4".to_string()));
        assert_eq!(config.max_output_tokens, Some(8192));
    }

    #[test]
    fn deserializes_empty_json_object() {
        let json = "{}";

        let config: ToolModelConfig = serde_json::from_str(json).unwrap();

        assert!(config.is_empty());
    }

    #[test]
    fn roundtrip_serialization() {
        let original = ToolModelConfig::new("anthropic", "claude-3").with_max_output_tokens(1024);

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ToolModelConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(original, deserialized);
    }
}
