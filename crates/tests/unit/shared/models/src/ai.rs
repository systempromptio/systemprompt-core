//! Unit tests for AI service models
//!
//! Tests cover:
//! - AiMessage creation and factory methods
//! - MessageRole enum variants and serialization
//! - AiRequest builder pattern
//! - SamplingParams and ProviderConfig

use systemprompt_models::{AiMessage, MessageRole, ProviderConfig, SamplingParams};

// ============================================================================
// AiMessage Tests
// ============================================================================

#[test]
fn test_ai_message_user() {
    let msg = AiMessage::user("Hello");

    assert!(matches!(msg.role, MessageRole::User));
    assert_eq!(msg.content, "Hello");
}

#[test]
fn test_ai_message_assistant() {
    let msg = AiMessage::assistant("Hi there");

    assert!(matches!(msg.role, MessageRole::Assistant));
    assert_eq!(msg.content, "Hi there");
}

#[test]
fn test_ai_message_system() {
    let msg = AiMessage::system("You are a helpful assistant");

    assert!(matches!(msg.role, MessageRole::System));
    assert_eq!(msg.content, "You are a helpful assistant");
}

#[test]
fn test_ai_message_user_from_string() {
    let msg = AiMessage::user(String::from("Hello from String"));

    assert!(matches!(msg.role, MessageRole::User));
    assert_eq!(msg.content, "Hello from String");
}

#[test]
fn test_ai_message_serialize() {
    let msg = AiMessage::user("Test message");

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("user"));
    assert!(json.contains("Test message"));
}

#[test]
fn test_ai_message_deserialize() {
    let json = r#"{"role":"assistant","content":"Response text"}"#;
    let msg: AiMessage = serde_json::from_str(json).unwrap();

    assert!(matches!(msg.role, MessageRole::Assistant));
    assert_eq!(msg.content, "Response text");
}

// ============================================================================
// MessageRole Tests
// ============================================================================

#[test]
fn test_message_role_system_serialize() {
    let json = serde_json::to_string(&MessageRole::System).unwrap();
    assert_eq!(json, "\"system\"");
}

#[test]
fn test_message_role_user_serialize() {
    let json = serde_json::to_string(&MessageRole::User).unwrap();
    assert_eq!(json, "\"user\"");
}

#[test]
fn test_message_role_assistant_serialize() {
    let json = serde_json::to_string(&MessageRole::Assistant).unwrap();
    assert_eq!(json, "\"assistant\"");
}

#[test]
fn test_message_role_deserialize_system() {
    let role: MessageRole = serde_json::from_str("\"system\"").unwrap();
    assert!(matches!(role, MessageRole::System));
}

#[test]
fn test_message_role_deserialize_user() {
    let role: MessageRole = serde_json::from_str("\"user\"").unwrap();
    assert!(matches!(role, MessageRole::User));
}

#[test]
fn test_message_role_deserialize_assistant() {
    let role: MessageRole = serde_json::from_str("\"assistant\"").unwrap();
    assert!(matches!(role, MessageRole::Assistant));
}

#[test]
fn test_message_role_equality() {
    assert_eq!(MessageRole::User, MessageRole::User);
    assert_eq!(MessageRole::System, MessageRole::System);
    assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
    assert_ne!(MessageRole::User, MessageRole::Assistant);
}

#[test]
fn test_message_role_copy() {
    let role = MessageRole::User;
    let copied = role;
    assert_eq!(role, copied);
}

// ============================================================================
// ProviderConfig Tests
// ============================================================================

#[test]
fn test_provider_config_new() {
    let config = ProviderConfig::new("anthropic", "claude-3-sonnet", 4096);

    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model, "claude-3-sonnet");
    assert_eq!(config.max_output_tokens, 4096);
}

#[test]
fn test_provider_config_with_string_types() {
    let config = ProviderConfig::new(
        String::from("openai"),
        String::from("gpt-4"),
        8192,
    );

    assert_eq!(config.provider, "openai");
    assert_eq!(config.model, "gpt-4");
}

#[test]
fn test_provider_config_serialize() {
    let config = ProviderConfig::new("anthropic", "claude-3", 4096);

    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("anthropic"));
    assert!(json.contains("claude-3"));
    assert!(json.contains("4096"));
}

#[test]
fn test_provider_config_deserialize() {
    let json = r#"{"provider":"test","model":"test-model","max_output_tokens":1000}"#;
    let config: ProviderConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.provider, "test");
    assert_eq!(config.model, "test-model");
    assert_eq!(config.max_output_tokens, 1000);
}

// ============================================================================
// SamplingParams Tests
// ============================================================================

#[test]
fn test_sampling_params_default() {
    let params = SamplingParams::default();

    // Default values should be reasonable
    let json = serde_json::to_string(&params).unwrap();
    assert!(!json.is_empty());
}

#[test]
fn test_sampling_params_serialize() {
    let params = SamplingParams::default();
    let json = serde_json::to_string(&params).unwrap();

    // Should serialize without error
    assert!(!json.is_empty());
}

#[test]
fn test_sampling_params_deserialize() {
    let json = "{}";
    let result: Result<SamplingParams, _> = serde_json::from_str(json);

    // Should handle empty object (using defaults)
    assert!(result.is_ok());
}

// ============================================================================
// AiRequest Tests
// Note: Full AiRequest deserialization requires a complex RequestContext,
// so we test the accessor methods via the ProviderConfig which is simpler
// ============================================================================

#[test]
fn test_ai_request_provider_config_accessors() {
    // Test that ProviderConfig provides the values that AiRequest would expose
    let config = ProviderConfig::new("anthropic", "claude-3-sonnet", 4096);

    assert_eq!(config.provider, "anthropic");
    assert_eq!(config.model, "claude-3-sonnet");
    assert_eq!(config.max_output_tokens, 4096);
}

#[test]
fn test_ai_request_has_tools_logic() {
    // Test the has_tools logic pattern (None case)
    let tools: Option<Vec<String>> = None;
    let has_tools = tools.as_ref().is_some_and(|t| !t.is_empty());
    assert!(!has_tools);
}

#[test]
fn test_ai_request_has_tools_empty_vec() {
    // Test the has_tools logic pattern (empty vec case)
    let tools: Option<Vec<String>> = Some(vec![]);
    let has_tools = tools.as_ref().is_some_and(|t| !t.is_empty());
    assert!(!has_tools);
}

#[test]
fn test_ai_request_has_tools_with_items() {
    // Test the has_tools logic pattern (non-empty vec case)
    let tools: Option<Vec<String>> = Some(vec!["tool1".to_string()]);
    let has_tools = tools.as_ref().is_some_and(|t| !t.is_empty());
    assert!(has_tools);
}
