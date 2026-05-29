use systemprompt_models::ai::{
    ModelConfig, ModelHint, ModelPreferences, ProviderConfig, ResponseFormat, SamplingParams,
    StructuredOutputOptions, ToolModelConfig,
};

#[test]
fn sampling_params_default_all_none() {
    let s = SamplingParams::default();
    assert!(s.temperature.is_none());
    assert!(s.top_p.is_none());
    assert!(s.top_k.is_none());
    assert!(s.presence_penalty.is_none());
    assert!(s.frequency_penalty.is_none());
    assert!(s.stop_sequences.is_none());
}

#[test]
fn sampling_params_serde_round_trip() {
    let s = SamplingParams {
        temperature: Some(0.7),
        top_p: Some(0.9),
        top_k: Some(50),
        presence_penalty: Some(0.1),
        frequency_penalty: Some(0.2),
        stop_sequences: Some(vec!["STOP".to_owned()]),
    };
    let json = serde_json::to_string(&s).unwrap();
    let decoded: SamplingParams = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.temperature, Some(0.7));
    assert_eq!(decoded.top_k, Some(50));
    assert_eq!(decoded.stop_sequences.as_ref().map(|v| v.len()), Some(1));
}

#[test]
fn model_preferences_default_is_empty() {
    let p = ModelPreferences::default();
    assert!(p.hints.is_empty());
    assert!(p.cost_priority.is_none());
}

#[test]
fn model_hint_serde_as_string() {
    let hint = ModelHint::ModelId("claude-3-opus".to_owned());
    let json = serde_json::to_string(&hint).unwrap();
    assert_eq!(json, "\"claude-3-opus\"");
    let decoded: ModelHint = serde_json::from_str(&json).unwrap();
    let ModelHint::ModelId(id) = decoded else { panic!("wrong variant") };
    assert_eq!(id, "claude-3-opus");
}

#[test]
fn provider_config_new_sets_fields() {
    let pc = ProviderConfig::new("anthropic", "claude-3-sonnet", 4096);
    assert_eq!(pc.provider, "anthropic");
    assert_eq!(pc.model, "claude-3-sonnet");
    assert_eq!(pc.max_output_tokens, 4096);
}

#[test]
fn provider_config_serde_round_trip() {
    let pc = ProviderConfig::new("openai", "gpt-4", 2048);
    let json = serde_json::to_string(&pc).unwrap();
    let decoded: ProviderConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.provider, "openai");
    assert_eq!(decoded.max_output_tokens, 2048);
}

#[test]
fn response_format_default_is_text() {
    let rf = ResponseFormat::default();
    assert!(matches!(rf, ResponseFormat::Text));
    assert!(!rf.is_json());
    assert!(rf.schema().is_none());
}

#[test]
fn response_format_json_object() {
    let rf = ResponseFormat::json_object();
    assert!(matches!(rf, ResponseFormat::JsonObject));
    assert!(rf.is_json());
    assert!(rf.schema().is_none());
}

#[test]
fn response_format_json_schema() {
    let schema = serde_json::json!({ "type": "object" });
    let rf = ResponseFormat::json_schema(schema.clone());
    assert!(rf.is_json());
    assert_eq!(rf.schema(), Some(&schema));
}

#[test]
fn response_format_json_schema_named() {
    let schema = serde_json::json!({ "type": "object" });
    let rf = ResponseFormat::json_schema_named(schema.clone(), "MySchema".to_owned());
    assert!(rf.is_json());
    if let ResponseFormat::JsonSchema { name, strict, .. } = &rf {
        assert_eq!(name.as_deref(), Some("MySchema"));
        assert_eq!(*strict, Some(true));
    } else {
        panic!("expected JsonSchema variant");
    }
}

#[test]
fn response_format_text_is_json_false() {
    let rf = ResponseFormat::Text;
    assert!(!rf.is_json());
}

#[test]
fn structured_output_options_new_is_default() {
    let opt = StructuredOutputOptions::new();
    assert!(opt.response_format.is_none());
    assert!(opt.max_retries.is_none());
}

#[test]
fn structured_output_options_with_json_object() {
    let opt = StructuredOutputOptions::with_json_object();
    assert!(matches!(
        opt.response_format,
        Some(ResponseFormat::JsonObject)
    ));
    assert_eq!(opt.inject_json_prompt, Some(true));
    assert_eq!(opt.validate_schema, Some(false));
}

#[test]
fn structured_output_options_with_schema() {
    let schema = serde_json::json!({ "type": "string" });
    let opt = StructuredOutputOptions::with_schema(schema);
    assert!(opt.inject_json_prompt == Some(true));
    assert_eq!(opt.max_retries, Some(3));
    assert_eq!(opt.validate_schema, Some(true));
    assert!(matches!(
        opt.response_format,
        Some(ResponseFormat::JsonSchema { .. })
    ));
}

#[test]
fn tool_model_config_new_sets_provider_and_model() {
    let c = ToolModelConfig::new("anthropic", "claude-sonnet");
    assert_eq!(c.provider.as_deref(), Some("anthropic"));
    assert_eq!(c.model.as_deref(), Some("claude-sonnet"));
    assert!(c.max_output_tokens.is_none());
    assert!(!c.is_empty());
}

#[test]
fn tool_model_config_with_max_output_tokens() {
    let c = ToolModelConfig::new("openai", "gpt-4").with_max_output_tokens(1024);
    assert_eq!(c.max_output_tokens, Some(1024));
}

#[test]
fn tool_model_config_default_is_empty() {
    let c = ToolModelConfig::default();
    assert!(c.is_empty());
}

#[test]
fn tool_model_config_merge_with_other_wins() {
    let base = ToolModelConfig::new("openai", "gpt-3.5");
    let override_cfg = ToolModelConfig::new("anthropic", "claude-3-haiku");
    let merged = base.merge_with(&override_cfg);
    assert_eq!(merged.provider.as_deref(), Some("anthropic"));
    assert_eq!(merged.model.as_deref(), Some("claude-3-haiku"));
}

#[test]
fn tool_model_config_merge_with_empty_other_keeps_base() {
    let base = ToolModelConfig::new("openai", "gpt-4");
    let empty = ToolModelConfig::default();
    let merged = base.merge_with(&empty);
    assert_eq!(merged.provider.as_deref(), Some("openai"));
    assert_eq!(merged.model.as_deref(), Some("gpt-4"));
}

#[test]
fn model_config_new_and_with_cost() {
    let mc = ModelConfig::new("gpt-4", 8192, true).with_cost(0.03);
    assert_eq!(mc.id, "gpt-4");
    assert_eq!(mc.max_tokens, 8192);
    assert!(mc.supports_tools);
    assert!((mc.cost_per_1k_tokens - 0.03).abs() < 1e-6);
}

#[test]
fn model_config_default_cost_zero() {
    let mc = ModelConfig::new("gpt-3.5", 4096, false);
    assert!((mc.cost_per_1k_tokens).abs() < 1e-6);
}
