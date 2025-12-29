//! Unit tests for AgentInfo model
//!
//! Tests cover:
//! - AgentInfo construction methods
//! - Getter methods (id, name, endpoint, version)
//! - Builder pattern methods (with_skills, with_mcp_servers)
//! - Count methods (skills_count, mcp_count)

use systemprompt_core_agent::models::agent_info::AgentInfo;
use systemprompt_core_agent::models::a2a::AgentCard;

fn create_test_card() -> AgentCard {
    AgentCard {
        protocol_version: "0.3.0".to_string(),
        name: "Test Agent".to_string(),
        description: "A test agent".to_string(),
        url: "/api/v1/agents/test".to_string(),
        version: "1.0.0".to_string(),
        preferred_transport: None,
        additional_interfaces: None,
        icon_url: None,
        provider: None,
        documentation_url: None,
        capabilities: Default::default(),
        security_schemes: None,
        security: None,
        default_input_modes: vec!["text/plain".to_string()],
        default_output_modes: vec!["text/plain".to_string()],
        skills: vec![],
        supports_authenticated_extended_card: None,
        signatures: None,
    }
}

// ============================================================================
// Construction Tests
// ============================================================================

#[test]
fn test_agent_info_from_repository_data() {
    let card = create_test_card();
    let info = AgentInfo::from_repository_data("agent-1".to_string(), card, true);

    assert_eq!(info.agent_id, "agent-1");
    assert!(info.enabled);
    assert!(info.skills.is_none());
    assert!(info.mcp_servers.is_none());
}

#[test]
fn test_agent_info_from_card() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-2".to_string(), card, false);

    assert_eq!(info.agent_id, "agent-2");
    assert!(!info.enabled);
    assert!(info.skills.is_none());
    assert!(info.mcp_servers.is_none());
}

// ============================================================================
// Getter Method Tests
// ============================================================================

#[test]
fn test_agent_info_id() {
    let card = create_test_card();
    let info = AgentInfo::from_card("test-id".to_string(), card, true);

    assert_eq!(info.id(), "test-id");
}

#[test]
fn test_agent_info_name() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    assert_eq!(info.name(), "Test Agent");
}

#[test]
fn test_agent_info_endpoint() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    assert_eq!(info.endpoint(), "/api/v1/agents/test");
}

#[test]
fn test_agent_info_version() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    assert_eq!(info.version(), "1.0.0");
}

// ============================================================================
// Builder Pattern Tests
// ============================================================================

#[test]
fn test_agent_info_with_skills() {
    use systemprompt_core_agent::models::a2a::AgentSkill;

    let card = create_test_card();
    let skills = vec![
        AgentSkill {
            id: "skill-1".to_string(),
            name: "Search".to_string(),
            description: "Search capability".to_string(),
            tags: vec!["search".to_string()],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
        AgentSkill {
            id: "skill-2".to_string(),
            name: "Chat".to_string(),
            description: "Chat capability".to_string(),
            tags: vec!["chat".to_string()],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
    ];

    let info = AgentInfo::from_card("agent-1".to_string(), card, true).with_skills(skills);

    assert!(info.skills.is_some());
    assert_eq!(info.skills.as_ref().unwrap().len(), 2);
}

#[test]
fn test_agent_info_with_mcp_servers() {
    let card = create_test_card();
    let servers = vec!["brave".to_string(), "postgres".to_string()];

    let info = AgentInfo::from_card("agent-1".to_string(), card, true).with_mcp_servers(servers);

    assert!(info.mcp_servers.is_some());
    assert_eq!(info.mcp_servers.as_ref().unwrap().len(), 2);
}

#[test]
fn test_agent_info_builder_chain() {
    use systemprompt_core_agent::models::a2a::AgentSkill;

    let card = create_test_card();
    let skills = vec![AgentSkill {
        id: "skill-1".to_string(),
        name: "Search".to_string(),
        description: "Search".to_string(),
        tags: vec![],
        examples: None,
        input_modes: None,
        output_modes: None,
        security: None,
    }];
    let servers = vec!["brave".to_string()];

    let info = AgentInfo::from_card("agent-1".to_string(), card, true)
        .with_skills(skills)
        .with_mcp_servers(servers);

    assert!(info.skills.is_some());
    assert!(info.mcp_servers.is_some());
}

// ============================================================================
// Count Method Tests
// ============================================================================

#[test]
fn test_agent_info_skills_count_none() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    assert_eq!(info.skills_count(), 0);
}

#[test]
fn test_agent_info_skills_count_with_skills() {
    use systemprompt_core_agent::models::a2a::AgentSkill;

    let card = create_test_card();
    let skills = vec![
        AgentSkill {
            id: "1".to_string(),
            name: "S1".to_string(),
            description: "D1".to_string(),
            tags: vec![],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
        AgentSkill {
            id: "2".to_string(),
            name: "S2".to_string(),
            description: "D2".to_string(),
            tags: vec![],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
        AgentSkill {
            id: "3".to_string(),
            name: "S3".to_string(),
            description: "D3".to_string(),
            tags: vec![],
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        },
    ];

    let info = AgentInfo::from_card("agent-1".to_string(), card, true).with_skills(skills);

    assert_eq!(info.skills_count(), 3);
}

#[test]
fn test_agent_info_mcp_count_none() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    assert_eq!(info.mcp_count(), 0);
}

#[test]
fn test_agent_info_mcp_count_with_servers() {
    let card = create_test_card();
    let servers = vec![
        "brave".to_string(),
        "postgres".to_string(),
        "filesystem".to_string(),
    ];

    let info = AgentInfo::from_card("agent-1".to_string(), card, true).with_mcp_servers(servers);

    assert_eq!(info.mcp_count(), 3);
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_agent_info_serialize() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("agent-1"));
    assert!(json.contains("Test Agent"));
    assert!(json.contains("1.0.0"));
}

#[test]
fn test_agent_info_deserialize() {
    let json = r#"{
        "agent_id": "agent-test",
        "card": {
            "protocolVersion": "0.3.0",
            "name": "Deserialized Agent",
            "description": "Test",
            "url": "/api/test",
            "version": "2.0.0",
            "capabilities": {},
            "defaultInputModes": ["text/plain"],
            "defaultOutputModes": ["text/plain"],
            "skills": []
        },
        "enabled": true
    }"#;

    let info: AgentInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.agent_id, "agent-test");
    assert_eq!(info.name(), "Deserialized Agent");
    assert!(info.enabled);
}

#[test]
fn test_agent_info_debug() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);

    let debug = format!("{:?}", info);
    assert!(debug.contains("AgentInfo"));
    assert!(debug.contains("agent-1"));
}

#[test]
fn test_agent_info_clone() {
    let card = create_test_card();
    let info = AgentInfo::from_card("agent-1".to_string(), card, true);
    let cloned = info.clone();

    assert_eq!(info.agent_id, cloned.agent_id);
    assert_eq!(info.enabled, cloned.enabled);
}
