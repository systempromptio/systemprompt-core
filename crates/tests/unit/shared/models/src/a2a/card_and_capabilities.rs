//! Tests for AgentCard, AgentCapabilities, and AgentProvider.

use systemprompt_models::{AgentCapabilities, AgentCard, AgentProvider};

// ============================================================================
// AgentCard Tests
// ============================================================================

#[test]
fn test_agent_card_builder_creates_valid_card() {
    let card = AgentCard::builder(
        "Test Agent".to_string(),
        "A test agent".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert_eq!(card.name, "Test Agent");
    assert_eq!(card.description, "A test agent");
    assert_eq!(card.url(), Some("https://example.com"));
    assert_eq!(card.version, "1.0.0");
}

#[test]
fn test_agent_card_builder_with_provider() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .with_provider(
        "systemprompt.io".to_string(),
        "https://systemprompt.io".to_string(),
    )
    .build();

    let provider = card.provider.expect("provider should be set");
    assert_eq!(provider.organization, "systemprompt.io");
    assert_eq!(provider.url, "https://systemprompt.io");
}

#[test]
fn test_agent_card_builder_with_streaming() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .with_streaming()
    .build();

    assert_eq!(card.capabilities.streaming, Some(true));
}

#[test]
fn test_agent_card_builder_with_push_notifications() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .with_push_notifications()
    .build();

    assert_eq!(card.capabilities.push_notifications, Some(true));
}

#[test]
fn test_agent_card_default_input_output_modes() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert!(card.default_input_modes.contains(&"text/plain".to_string()));
    assert!(
        card.default_output_modes
            .contains(&"text/plain".to_string())
    );
}

#[test]
fn test_agent_card_has_mcp_extension_false() {
    let card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert!(!card.has_mcp_extension());
}

#[test]
fn test_agent_card_ensure_mcp_extension() {
    let mut card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    assert!(!card.has_mcp_extension());
    card.ensure_mcp_extension();
    assert!(card.has_mcp_extension());
}

#[test]
fn test_agent_card_ensure_mcp_extension_idempotent() {
    let mut card = AgentCard::builder(
        "Test".to_string(),
        "Desc".to_string(),
        "https://example.com".to_string(),
        "1.0.0".to_string(),
    )
    .build();

    card.ensure_mcp_extension();
    let ext_count = card
        .capabilities
        .extensions
        .as_ref()
        .map(|e| e.len())
        .unwrap_or(0);

    card.ensure_mcp_extension();
    let ext_count_after = card
        .capabilities
        .extensions
        .as_ref()
        .map(|e| e.len())
        .unwrap_or(0);

    assert_eq!(ext_count, ext_count_after);
}

// ============================================================================
// AgentCapabilities Tests
// ============================================================================

#[test]
fn test_agent_capabilities_default() {
    let caps = AgentCapabilities::default();

    assert_eq!(caps.streaming, Some(true));
    assert_eq!(caps.push_notifications, Some(true));
    assert_eq!(caps.state_transition_history, Some(true));
    assert!(caps.extensions.is_none());
}

#[test]
fn test_agent_capabilities_normalize_none_values() {
    let caps = AgentCapabilities {
        streaming: None,
        push_notifications: None,
        state_transition_history: None,
        extensions: None,
    };

    let normalized = caps.normalize();

    assert_eq!(normalized.streaming, Some(true));
    assert_eq!(normalized.push_notifications, Some(false));
    assert_eq!(normalized.state_transition_history, Some(true));
}

#[test]
fn test_agent_capabilities_normalize_preserves_existing() {
    let caps = AgentCapabilities {
        streaming: Some(false),
        push_notifications: Some(true),
        state_transition_history: Some(false),
        extensions: None,
    };

    let normalized = caps.normalize();

    assert_eq!(normalized.streaming, Some(false));
    assert_eq!(normalized.push_notifications, Some(true));
    assert_eq!(normalized.state_transition_history, Some(false));
}

// ============================================================================
// AgentProvider Tests
// ============================================================================

#[test]
fn test_agent_provider_serialize() {
    let provider = AgentProvider {
        organization: "TestOrg".to_string(),
        url: "https://test.org".to_string(),
    };

    let json = serde_json::to_string(&provider).unwrap();
    assert!(json.contains("TestOrg"));
    assert!(json.contains("https://test.org"));
}
