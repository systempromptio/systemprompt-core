use std::collections::HashMap;
use systemprompt_models::services::{
    AiConfig, AiProviderConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition,
    ModelLimits, ModelPricing, SamplingConfig, ToolModelConfig, ToolModelSettings,
};

mod ai_config_defaults {
    use super::*;

    #[test]
    fn default_ai_config_has_empty_provider() {
        let config = AiConfig::default();
        assert!(config.default_provider.is_empty());
    }

    #[test]
    fn default_ai_config_has_no_max_output_tokens() {
        let config = AiConfig::default();
        assert!(config.default_max_output_tokens.is_none());
    }

    #[test]
    fn default_ai_config_has_empty_providers_map() {
        let config = AiConfig::default();
        assert!(config.providers.is_empty());
    }

    #[test]
    fn default_ai_config_has_empty_tool_models_map() {
        let config = AiConfig::default();
        assert!(config.tool_models.is_empty());
    }
}

mod ai_config_serde {
    use super::*;

    #[test]
    fn roundtrip_minimal_config() {
        let config = AiConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: AiConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.default_provider, config.default_provider);
    }

    #[test]
    fn deserialize_from_empty_object() {
        let config: AiConfig = serde_json::from_str("{}").expect("deserialize");
        assert!(config.default_provider.is_empty());
        assert!(config.providers.is_empty());
    }

    #[test]
    fn roundtrip_full_config() {
        let mut providers = HashMap::new();
        providers.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "sk-test".to_string(),
                endpoint: Some("https://api.openai.com".to_string()),
                default_model: "gpt-4".to_string(),
                default_image_model: "dall-e-3".to_string(),
                default_image_resolution: "1024x1024".to_string(),
                google_search_enabled: false,
                models: HashMap::new(),
            },
        );

        let config = AiConfig {
            default_provider: "openai".to_string(),
            default_max_output_tokens: Some(4096),
            providers,
            tool_models: HashMap::new(),
            sampling: SamplingConfig {
                enable_smart_routing: true,
                fallback_enabled: false,
            },
            mcp: McpConfig {
                auto_discover: true,
                connect_timeout_ms: 10000,
                execution_timeout_ms: 60000,
                retry_attempts: 5,
            },
            history: HistoryConfig {
                retention_days: 90,
                log_tool_executions: true,
            },
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: AiConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.default_provider, "openai");
        assert_eq!(deserialized.default_max_output_tokens, Some(4096));
        assert!(deserialized.providers.contains_key("openai"));
        assert!(deserialized.sampling.enable_smart_routing);
        assert!(!deserialized.sampling.fallback_enabled);
        assert!(deserialized.mcp.auto_discover);
        assert_eq!(deserialized.mcp.connect_timeout_ms, 10000);
        assert_eq!(deserialized.history.retention_days, 90);
        assert!(deserialized.history.log_tool_executions);
    }
}

mod sampling_config_tests {
    use super::*;

    #[test]
    fn default_has_both_disabled() {
        let config = SamplingConfig::default();
        assert!(!config.enable_smart_routing);
        assert!(!config.fallback_enabled);
    }

    #[test]
    fn serde_roundtrip() {
        let config = SamplingConfig {
            enable_smart_routing: true,
            fallback_enabled: true,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: SamplingConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.enable_smart_routing);
        assert!(deserialized.fallback_enabled);
    }
}

mod mcp_config_tests {
    use super::*;

    #[test]
    fn default_has_sensible_timeouts() {
        let config = McpConfig::default();
        assert_eq!(config.connect_timeout_ms, 5000);
        assert_eq!(config.execution_timeout_ms, 30000);
        assert_eq!(config.retry_attempts, 3);
        assert!(!config.auto_discover);
    }

    #[test]
    fn serde_roundtrip() {
        let config = McpConfig {
            auto_discover: true,
            connect_timeout_ms: 7000,
            execution_timeout_ms: 45000,
            retry_attempts: 1,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: McpConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.auto_discover);
        assert_eq!(deserialized.connect_timeout_ms, 7000);
        assert_eq!(deserialized.execution_timeout_ms, 45000);
        assert_eq!(deserialized.retry_attempts, 1);
    }

    #[test]
    fn deserialize_with_defaults_for_missing_fields() {
        let config: McpConfig = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(config.connect_timeout_ms, 5000);
        assert_eq!(config.execution_timeout_ms, 30000);
        assert_eq!(config.retry_attempts, 3);
    }
}

mod history_config_tests {
    use super::*;

    #[test]
    fn default_has_thirty_day_retention() {
        let config = HistoryConfig::default();
        assert_eq!(config.retention_days, 30);
        assert!(!config.log_tool_executions);
    }

    #[test]
    fn serde_roundtrip() {
        let config = HistoryConfig {
            retention_days: 365,
            log_tool_executions: true,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: HistoryConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.retention_days, 365);
        assert!(deserialized.log_tool_executions);
    }
}

mod ai_provider_config_tests {
    use super::*;

    #[test]
    fn default_is_enabled_with_empty_fields() {
        let config = AiProviderConfig::default();
        assert!(config.enabled);
        assert!(config.api_key.is_empty());
        assert!(config.endpoint.is_none());
        assert!(config.default_model.is_empty());
        assert!(config.default_image_model.is_empty());
        assert!(config.default_image_resolution.is_empty());
        assert!(!config.google_search_enabled);
        assert!(config.models.is_empty());
    }

    #[test]
    fn serde_enabled_defaults_to_true() {
        let config: AiProviderConfig =
            serde_json::from_str(r#"{"api_key":"test"}"#).expect("deserialize");
        assert!(config.enabled);
    }

    #[test]
    fn serde_roundtrip_with_models() {
        let mut models = HashMap::new();
        models.insert(
            "gpt-4".to_string(),
            ModelDefinition {
                capabilities: ModelCapabilities {
                    vision: true,
                    tools: true,
                    structured_output: true,
                    ..Default::default()
                },
                limits: ModelLimits {
                    context_window: 128000,
                    max_output_tokens: 8192,
                },
                pricing: ModelPricing {
                    input_per_million: 30.0,
                    output_per_million: 60.0,
                    per_image_cents: Some(1.0),
                },
            },
        );

        let config = AiProviderConfig {
            enabled: true,
            api_key: "sk-test".to_string(),
            endpoint: None,
            default_model: "gpt-4".to_string(),
            default_image_model: String::new(),
            default_image_resolution: String::new(),
            google_search_enabled: false,
            models,
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: AiProviderConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.models.contains_key("gpt-4"));
        let model = &deserialized.models["gpt-4"];
        assert!(model.capabilities.vision);
        assert_eq!(model.limits.context_window, 128000);
        assert!((model.pricing.input_per_million - 30.0).abs() < f64::EPSILON);
    }
}

mod model_capabilities_tests {
    use super::*;

    #[test]
    fn default_all_false() {
        let caps = ModelCapabilities::default();
        assert!(!caps.vision);
        assert!(!caps.audio_input);
        assert!(!caps.video_input);
        assert!(!caps.image_generation);
        assert!(!caps.audio_generation);
        assert!(!caps.streaming);
        assert!(!caps.tools);
        assert!(!caps.structured_output);
        assert!(!caps.system_prompts);
        assert!(!caps.image_resolution_config);
    }

    #[test]
    fn serde_roundtrip() {
        let caps = ModelCapabilities {
            vision: true,
            audio_input: true,
            video_input: false,
            image_generation: false,
            audio_generation: false,
            streaming: true,
            tools: true,
            structured_output: true,
            system_prompts: true,
            image_resolution_config: false,
        };
        let json = serde_json::to_string(&caps).expect("serialize");
        let deserialized: ModelCapabilities = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.vision);
        assert!(deserialized.audio_input);
        assert!(deserialized.streaming);
        assert!(deserialized.tools);
        assert!(deserialized.structured_output);
        assert!(deserialized.system_prompts);
        assert!(!deserialized.video_input);
    }
}

mod model_limits_tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let limits = ModelLimits::default();
        assert_eq!(limits.context_window, 0);
        assert_eq!(limits.max_output_tokens, 0);
    }

    #[test]
    fn serde_roundtrip() {
        let limits = ModelLimits {
            context_window: 200000,
            max_output_tokens: 16384,
        };
        let json = serde_json::to_string(&limits).expect("serialize");
        let deserialized: ModelLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.context_window, 200000);
        assert_eq!(deserialized.max_output_tokens, 16384);
    }
}

mod model_pricing_tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let pricing = ModelPricing::default();
        assert!((pricing.input_per_million - 0.0).abs() < f64::EPSILON);
        assert!((pricing.output_per_million - 0.0).abs() < f64::EPSILON);
        assert!(pricing.per_image_cents.is_none());
    }

    #[test]
    fn serde_roundtrip_with_image_cost() {
        let pricing = ModelPricing {
            input_per_million: 15.0,
            output_per_million: 75.0,
            per_image_cents: Some(4.0),
        };
        let json = serde_json::to_string(&pricing).expect("serialize");
        let deserialized: ModelPricing = serde_json::from_str(&json).expect("deserialize");
        assert!((deserialized.input_per_million - 15.0).abs() < f64::EPSILON);
        assert!((deserialized.output_per_million - 75.0).abs() < f64::EPSILON);
        assert_eq!(deserialized.per_image_cents, Some(4.0));
    }

    #[test]
    fn serde_roundtrip_without_image_cost() {
        let pricing = ModelPricing {
            input_per_million: 1.0,
            output_per_million: 2.0,
            per_image_cents: None,
        };
        let json = serde_json::to_string(&pricing).expect("serialize");
        let deserialized: ModelPricing = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.per_image_cents.is_none());
    }
}

mod tool_model_settings_tests {
    use super::*;

    #[test]
    fn default_has_empty_model() {
        let settings = ToolModelSettings::default();
        assert!(settings.model.is_empty());
        assert!(settings.max_output_tokens.is_none());
    }

    #[test]
    fn serde_roundtrip() {
        let settings = ToolModelSettings {
            model: "gpt-4-turbo".to_string(),
            max_output_tokens: Some(2048),
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        let deserialized: ToolModelSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.model, "gpt-4-turbo");
        assert_eq!(deserialized.max_output_tokens, Some(2048));
    }
}

mod tool_model_config_tests {
    use super::*;

    #[test]
    fn serde_roundtrip() {
        let config = ToolModelConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            max_output_tokens: Some(4096),
            thinking_level: Some("high".to_string()),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: ToolModelConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.provider, "anthropic");
        assert_eq!(deserialized.model, "claude-3-opus");
        assert_eq!(deserialized.max_output_tokens, Some(4096));
        assert_eq!(deserialized.thinking_level.as_deref(), Some("high"));
    }

    #[test]
    fn serde_skips_none_fields() {
        let config = ToolModelConfig {
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            max_output_tokens: None,
            thinking_level: None,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        assert!(!json.contains("max_output_tokens"));
        assert!(!json.contains("thinking_level"));
    }

    #[test]
    fn deserialize_minimal() {
        let config: ToolModelConfig =
            serde_json::from_str(r#"{"provider":"gemini","model":"gemini-pro"}"#)
                .expect("deserialize");
        assert_eq!(config.provider, "gemini");
        assert_eq!(config.model, "gemini-pro");
        assert!(config.max_output_tokens.is_none());
        assert!(config.thinking_level.is_none());
    }
}

mod model_definition_tests {
    use super::*;

    #[test]
    fn default_all_zero() {
        let def = ModelDefinition::default();
        assert!(!def.capabilities.vision);
        assert_eq!(def.limits.context_window, 0);
        assert!((def.pricing.input_per_million - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn serde_roundtrip() {
        let def = ModelDefinition {
            capabilities: ModelCapabilities {
                vision: true,
                streaming: true,
                ..Default::default()
            },
            limits: ModelLimits {
                context_window: 1000000,
                max_output_tokens: 65536,
            },
            pricing: ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                per_image_cents: None,
            },
        };
        let json = serde_json::to_string(&def).expect("serialize");
        let deserialized: ModelDefinition = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.capabilities.vision);
        assert!(deserialized.capabilities.streaming);
        assert_eq!(deserialized.limits.context_window, 1000000);
        assert!((deserialized.pricing.output_per_million - 15.0).abs() < f64::EPSILON);
    }
}
