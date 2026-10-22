use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::{
    AiConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition, ModelPricing,
    SamplingConfig,
};
use systemprompt_wire::ModelLimits;

mod ai_config_defaults {
    use super::*;

    #[test]
    fn default_ai_config_has_no_image_model() {
        let config = AiConfig::default();
        assert!(config.default_image_model.is_none());
    }

    #[test]
    fn default_ai_config_has_no_max_output_tokens() {
        let config = AiConfig::default();
        assert!(config.default_max_output_tokens.is_none());
    }
}

mod ai_config_serde {
    use super::*;

    #[test]
    fn deserialize_from_empty_object() {
        let config: AiConfig = serde_json::from_str("{}").expect("deserialize");
        assert!(config.default_image_model.is_none());
        assert!(config.default_max_output_tokens.is_none());
    }

    #[test]
    fn the_retired_provider_keys_are_refused() {
        for retired in [
            r#"{"default_provider":"openai"}"#,
            r#"{"providers":{"openai":{"enabled":true}}}"#,
        ] {
            let err = serde_json::from_str::<AiConfig>(retired).unwrap_err();
            assert!(
                err.to_string().contains("unknown field"),
                "{retired}: {err}"
            );
        }
    }

    #[test]
    fn roundtrip_full_config() {
        let config = AiConfig {
            default_max_output_tokens: Some(4096),
            default_image_model: Some(ModelId::new("gemini-2.5-flash-image")),
            sampling: SamplingConfig {
                enable_smart_routing: true,
                fallback_enabled: false,
            },
            mcp: McpConfig {
                auto_discover: true,
                ..McpConfig::default()
            },
            history: HistoryConfig {
                retention_days: 90,
                log_tool_executions: true,
            },
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: AiConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.default_max_output_tokens, Some(4096));
        assert_eq!(
            deserialized
                .default_image_model
                .as_ref()
                .map(ModelId::as_str),
            Some("gemini-2.5-flash-image")
        );
        assert!(deserialized.sampling.enable_smart_routing);
        assert!(!deserialized.sampling.fallback_enabled);
        assert!(deserialized.mcp.auto_discover);
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
        assert_eq!(config.resilience.connect_timeout_ms, 5000);
        assert_eq!(config.resilience.request_timeout_ms, 30000);
        assert_eq!(config.resilience.retry_attempts, 3);
        assert!(!config.auto_discover);
    }

    #[test]
    fn serde_roundtrip() {
        let config = McpConfig {
            auto_discover: true,
            ..McpConfig::default()
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: McpConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.auto_discover);
        assert_eq!(deserialized.resilience.request_timeout_ms, 30000);
    }

    #[test]
    fn deserialize_with_defaults_for_missing_fields() {
        let config: McpConfig = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(config.resilience.connect_timeout_ms, 5000);
        assert_eq!(config.resilience.request_timeout_ms, 30000);
        assert_eq!(config.resilience.retry_attempts, 3);
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
            reasoning: true,
            prompt_caching: true,
        };
        let json = serde_json::to_string(&caps).expect("serialize");
        let deserialized: ModelCapabilities = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.vision);
        assert!(deserialized.audio_input);
        assert!(deserialized.streaming);
        assert!(deserialized.tools);
        assert!(deserialized.reasoning);
        assert!(deserialized.structured_output);
        assert!(deserialized.prompt_caching);
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
            max_thinking_budget: Some(24576),
        };
        let json = serde_json::to_string(&limits).expect("serialize");
        let deserialized: ModelLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.context_window, 200000);
        assert_eq!(deserialized.max_output_tokens, 16384);
        assert_eq!(deserialized.max_thinking_budget, Some(24576));
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
            ..ModelPricing::default()
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
            ..ModelPricing::default()
        };
        let json = serde_json::to_string(&pricing).expect("serialize");
        let deserialized: ModelPricing = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.per_image_cents.is_none());
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
                max_thinking_budget: None,
            },
            pricing: ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                ..ModelPricing::default()
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
