//! Unit tests for web models
//!
//! Tests cover:
//! - ListAgentsQuery default values and serialization
//! - AgentDiscovery types
//! - AgentCounts structure

use systemprompt_agent::models::web::{
    AgentCounts, AgentDiscoveryEntry, AgentDiscoveryResponse, ListAgentsQuery,
};

// ============================================================================
// ListAgentsQuery Tests
// ============================================================================

#[test]
fn test_list_agents_query_default() {
    let query = ListAgentsQuery::default();

    assert_eq!(query.page, Some(1));
    assert_eq!(query.limit, Some(20));
    assert_eq!(query.offset, Some(0));
    assert!(query.search.is_none());
    assert!(query.status.is_none());
    assert!(query.capability.is_none());
}

#[test]
fn test_list_agents_query_serialize() {
    let query = ListAgentsQuery {
        page: Some(2),
        limit: Some(50),
        offset: Some(10),
        search: Some("test".to_string()),
        status: Some("running".to_string()),
        capability: Some("streaming".to_string()),
    };

    let json = serde_json::to_string(&query).unwrap();
    assert!(json.contains("\"page\":2"));
    assert!(json.contains("\"limit\":50"));
    assert!(json.contains("\"offset\":10"));
    assert!(json.contains("test"));
    assert!(json.contains("running"));
    assert!(json.contains("streaming"));
}

#[test]
fn test_list_agents_query_deserialize() {
    let json = r#"{
        "page": 3,
        "limit": 100,
        "offset": 25,
        "search": "agent",
        "status": "stopped",
        "capability": "push_notifications"
    }"#;

    let query: ListAgentsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.page, Some(3));
    assert_eq!(query.limit, Some(100));
    assert_eq!(query.offset, Some(25));
    assert_eq!(query.search, Some("agent".to_string()));
    assert_eq!(query.status, Some("stopped".to_string()));
    assert_eq!(query.capability, Some("push_notifications".to_string()));
}

#[test]
fn test_list_agents_query_partial_deserialize() {
    let json = r#"{"page": 1}"#;

    let query: ListAgentsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.page, Some(1));
    assert!(query.limit.is_none());
    assert!(query.offset.is_none());
    assert!(query.search.is_none());
}

#[test]
fn test_list_agents_query_empty_deserialize() {
    let json = r#"{}"#;

    let query: ListAgentsQuery = serde_json::from_str(json).unwrap();
    assert!(query.page.is_none());
    assert!(query.limit.is_none());
    assert!(query.offset.is_none());
}

#[test]
fn test_list_agents_query_debug() {
    let query = ListAgentsQuery::default();
    let debug_str = format!("{:?}", query);
    assert!(debug_str.contains("ListAgentsQuery"));
}

#[test]
fn test_list_agents_query_clone() {
    let query = ListAgentsQuery {
        page: Some(5),
        limit: Some(10),
        offset: Some(20),
        search: Some("clone".to_string()),
        status: None,
        capability: None,
    };

    let cloned = query.clone();
    assert_eq!(cloned.page, query.page);
    assert_eq!(cloned.limit, query.limit);
    assert_eq!(cloned.search, query.search);
}

// ============================================================================
// AgentCounts Tests
// ============================================================================

#[test]
fn test_agent_counts_serialize() {
    let counts = AgentCounts {
        total: 10,
        active: 5,
        enabled: 8,
    };

    let json = serde_json::to_string(&counts).unwrap();
    assert!(json.contains("\"total\":10"));
    assert!(json.contains("\"active\":5"));
    assert!(json.contains("\"enabled\":8"));
}

#[test]
fn test_agent_counts_deserialize() {
    let json = r#"{"total": 20, "active": 15, "enabled": 18}"#;

    let counts: AgentCounts = serde_json::from_str(json).unwrap();
    assert_eq!(counts.total, 20);
    assert_eq!(counts.active, 15);
    assert_eq!(counts.enabled, 18);
}

#[test]
fn test_agent_counts_zero_values() {
    let counts = AgentCounts {
        total: 0,
        active: 0,
        enabled: 0,
    };

    let json = serde_json::to_string(&counts).unwrap();
    let deserialized: AgentCounts = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.total, 0);
    assert_eq!(deserialized.active, 0);
    assert_eq!(deserialized.enabled, 0);
}

#[test]
fn test_agent_counts_debug() {
    let counts = AgentCounts {
        total: 5,
        active: 3,
        enabled: 4,
    };

    let debug_str = format!("{:?}", counts);
    assert!(debug_str.contains("AgentCounts"));
    assert!(debug_str.contains("5"));
    assert!(debug_str.contains("3"));
    assert!(debug_str.contains("4"));
}

#[test]
fn test_agent_counts_copy() {
    let counts = AgentCounts {
        total: 100,
        active: 50,
        enabled: 75,
    };

    let copied = counts;
    assert_eq!(copied.total, 100);
    assert_eq!(copied.active, 50);
    assert_eq!(copied.enabled, 75);
}

// ============================================================================
// AgentDiscoveryEntry Tests
// ============================================================================

#[test]
fn test_agent_discovery_entry_serialize() {
    let entry = AgentDiscoveryEntry {
        uuid: "uuid-123".to_string(),
        slug: "test-agent".to_string(),
        name: "Test Agent".to_string(),
        description: "A test agent for testing".to_string(),
        version: "1.0.0".to_string(),
        url: "https://example.com/agent".to_string(),
        status: "running".to_string(),
        endpoint: "http://localhost:8080".to_string(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("uuid-123"));
    assert!(json.contains("test-agent"));
    assert!(json.contains("Test Agent"));
    assert!(json.contains("1.0.0"));
    assert!(json.contains("running"));
}

#[test]
fn test_agent_discovery_entry_deserialize() {
    let json = r#"{
        "uuid": "uuid-456",
        "slug": "my-agent",
        "name": "My Agent",
        "description": "Description here",
        "version": "2.0.0",
        "url": "https://agent.example.com",
        "status": "stopped",
        "endpoint": "http://127.0.0.1:9000"
    }"#;

    let entry: AgentDiscoveryEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.uuid, "uuid-456");
    assert_eq!(entry.slug, "my-agent");
    assert_eq!(entry.name, "My Agent");
    assert_eq!(entry.version, "2.0.0");
    assert_eq!(entry.status, "stopped");
}

#[test]
fn test_agent_discovery_entry_debug() {
    let entry = AgentDiscoveryEntry {
        uuid: "debug-uuid".to_string(),
        slug: "debug-slug".to_string(),
        name: "Debug Agent".to_string(),
        description: "Debug description".to_string(),
        version: "0.1.0".to_string(),
        url: "http://debug".to_string(),
        status: "debug".to_string(),
        endpoint: "http://localhost".to_string(),
    };

    let debug_str = format!("{:?}", entry);
    assert!(debug_str.contains("AgentDiscoveryEntry"));
    assert!(debug_str.contains("debug-uuid"));
}

#[test]
fn test_agent_discovery_entry_clone() {
    let entry = AgentDiscoveryEntry {
        uuid: "clone-uuid".to_string(),
        slug: "clone-slug".to_string(),
        name: "Clone Agent".to_string(),
        description: "Cloned".to_string(),
        version: "3.0.0".to_string(),
        url: "http://clone".to_string(),
        status: "active".to_string(),
        endpoint: "http://clone-endpoint".to_string(),
    };

    let cloned = entry.clone();
    assert_eq!(cloned.uuid, entry.uuid);
    assert_eq!(cloned.slug, entry.slug);
    assert_eq!(cloned.name, entry.name);
}

// ============================================================================
// AgentDiscoveryResponse Tests
// ============================================================================

#[test]
fn test_agent_discovery_response_serialize() {
    let response = AgentDiscoveryResponse {
        agents: vec![AgentDiscoveryEntry {
            uuid: "agent-1".to_string(),
            slug: "first".to_string(),
            name: "First Agent".to_string(),
            description: "First".to_string(),
            version: "1.0.0".to_string(),
            url: "http://first".to_string(),
            status: "running".to_string(),
            endpoint: "http://first:8080".to_string(),
        }],
        total: 1,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("agents"));
    assert!(json.contains("agent-1"));
    assert!(json.contains("\"total\":1"));
}

#[test]
fn test_agent_discovery_response_empty() {
    let response = AgentDiscoveryResponse {
        agents: vec![],
        total: 0,
    };

    let json = serde_json::to_string(&response).unwrap();
    let deserialized: AgentDiscoveryResponse = serde_json::from_str(&json).unwrap();

    assert!(deserialized.agents.is_empty());
    assert_eq!(deserialized.total, 0);
}

#[test]
fn test_agent_discovery_response_multiple_agents() {
    let response = AgentDiscoveryResponse {
        agents: vec![
            AgentDiscoveryEntry {
                uuid: "a1".to_string(),
                slug: "agent-1".to_string(),
                name: "Agent 1".to_string(),
                description: "First".to_string(),
                version: "1.0.0".to_string(),
                url: "http://a1".to_string(),
                status: "running".to_string(),
                endpoint: "http://a1:8080".to_string(),
            },
            AgentDiscoveryEntry {
                uuid: "a2".to_string(),
                slug: "agent-2".to_string(),
                name: "Agent 2".to_string(),
                description: "Second".to_string(),
                version: "2.0.0".to_string(),
                url: "http://a2".to_string(),
                status: "stopped".to_string(),
                endpoint: "http://a2:8080".to_string(),
            },
        ],
        total: 2,
    };

    assert_eq!(response.agents.len(), 2);
    assert_eq!(response.total, 2);
}

#[test]
fn test_agent_discovery_response_deserialize() {
    let json = r#"{
        "agents": [
            {
                "uuid": "u1",
                "slug": "s1",
                "name": "N1",
                "description": "D1",
                "version": "1.0",
                "url": "http://1",
                "status": "ok",
                "endpoint": "http://e1"
            }
        ],
        "total": 1
    }"#;

    let response: AgentDiscoveryResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.agents.len(), 1);
    assert_eq!(response.total, 1);
    assert_eq!(response.agents[0].uuid, "u1");
}
