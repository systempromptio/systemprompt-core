//! Unit tests for event system models
//!
//! Tests cover:
//! - SystemEventType enum variants and serialization
//! - SystemEventType as_str method
//! - A2AEventType serialization

use systemprompt_models::{SystemEventType, A2AEventType};

// ============================================================================
// SystemEventType Tests
// ============================================================================

#[test]
fn test_system_event_type_context_created_serialize() {
    let json = serde_json::to_string(&SystemEventType::ContextCreated).unwrap();
    assert_eq!(json, "\"CONTEXT_CREATED\"");
}

#[test]
fn test_system_event_type_context_updated_serialize() {
    let json = serde_json::to_string(&SystemEventType::ContextUpdated).unwrap();
    assert_eq!(json, "\"CONTEXT_UPDATED\"");
}

#[test]
fn test_system_event_type_context_deleted_serialize() {
    let json = serde_json::to_string(&SystemEventType::ContextDeleted).unwrap();
    assert_eq!(json, "\"CONTEXT_DELETED\"");
}

#[test]
fn test_system_event_type_contexts_snapshot_serialize() {
    let json = serde_json::to_string(&SystemEventType::ContextsSnapshot).unwrap();
    assert_eq!(json, "\"CONTEXTS_SNAPSHOT\"");
}

#[test]
fn test_system_event_type_connected_serialize() {
    let json = serde_json::to_string(&SystemEventType::Connected).unwrap();
    assert_eq!(json, "\"CONNECTED\"");
}

#[test]
fn test_system_event_type_heartbeat_serialize() {
    let json = serde_json::to_string(&SystemEventType::Heartbeat).unwrap();
    assert_eq!(json, "\"HEARTBEAT\"");
}

#[test]
fn test_system_event_type_deserialize_context_created() {
    let t: SystemEventType = serde_json::from_str("\"CONTEXT_CREATED\"").unwrap();
    assert!(matches!(t, SystemEventType::ContextCreated));
}

#[test]
fn test_system_event_type_deserialize_context_updated() {
    let t: SystemEventType = serde_json::from_str("\"CONTEXT_UPDATED\"").unwrap();
    assert!(matches!(t, SystemEventType::ContextUpdated));
}

#[test]
fn test_system_event_type_deserialize_context_deleted() {
    let t: SystemEventType = serde_json::from_str("\"CONTEXT_DELETED\"").unwrap();
    assert!(matches!(t, SystemEventType::ContextDeleted));
}

#[test]
fn test_system_event_type_deserialize_contexts_snapshot() {
    let t: SystemEventType = serde_json::from_str("\"CONTEXTS_SNAPSHOT\"").unwrap();
    assert!(matches!(t, SystemEventType::ContextsSnapshot));
}

#[test]
fn test_system_event_type_deserialize_connected() {
    let t: SystemEventType = serde_json::from_str("\"CONNECTED\"").unwrap();
    assert!(matches!(t, SystemEventType::Connected));
}

#[test]
fn test_system_event_type_deserialize_heartbeat() {
    let t: SystemEventType = serde_json::from_str("\"HEARTBEAT\"").unwrap();
    assert!(matches!(t, SystemEventType::Heartbeat));
}

#[test]
fn test_system_event_type_as_str_context_created() {
    let t = SystemEventType::ContextCreated;
    assert_eq!(t.as_str(), "CONTEXT_CREATED");
}

#[test]
fn test_system_event_type_as_str_context_updated() {
    let t = SystemEventType::ContextUpdated;
    assert_eq!(t.as_str(), "CONTEXT_UPDATED");
}

#[test]
fn test_system_event_type_as_str_context_deleted() {
    let t = SystemEventType::ContextDeleted;
    assert_eq!(t.as_str(), "CONTEXT_DELETED");
}

#[test]
fn test_system_event_type_as_str_contexts_snapshot() {
    let t = SystemEventType::ContextsSnapshot;
    assert_eq!(t.as_str(), "CONTEXTS_SNAPSHOT");
}

#[test]
fn test_system_event_type_as_str_connected() {
    let t = SystemEventType::Connected;
    assert_eq!(t.as_str(), "CONNECTED");
}

#[test]
fn test_system_event_type_as_str_heartbeat() {
    let t = SystemEventType::Heartbeat;
    assert_eq!(t.as_str(), "HEARTBEAT");
}

#[test]
fn test_system_event_type_equality() {
    assert_eq!(SystemEventType::ContextCreated, SystemEventType::ContextCreated);
    assert_eq!(SystemEventType::Connected, SystemEventType::Connected);
    assert_ne!(SystemEventType::ContextCreated, SystemEventType::ContextUpdated);
}

#[test]
fn test_system_event_type_copy() {
    let t = SystemEventType::Heartbeat;
    let copied = t;
    assert_eq!(t, copied);
}

#[test]
fn test_system_event_type_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(SystemEventType::ContextCreated);
    set.insert(SystemEventType::ContextUpdated);
    set.insert(SystemEventType::ContextCreated); // duplicate

    assert_eq!(set.len(), 2);
    assert!(set.contains(&SystemEventType::ContextCreated));
    assert!(set.contains(&SystemEventType::ContextUpdated));
}

#[test]
fn test_system_event_type_debug() {
    let t = SystemEventType::Connected;
    let debug_str = format!("{:?}", t);
    assert!(debug_str.contains("Connected"));
}

// ============================================================================
// A2AEventType Tests
// ============================================================================

#[test]
fn test_a2a_event_type_task_status_update_serialize() {
    let json = serde_json::to_string(&A2AEventType::TaskStatusUpdate).unwrap();
    assert_eq!(json, "\"TASK_STATUS_UPDATE\"");
}

#[test]
fn test_a2a_event_type_artifact_updated_serialize() {
    let json = serde_json::to_string(&A2AEventType::ArtifactUpdated).unwrap();
    assert_eq!(json, "\"ARTIFACT_UPDATED\"");
}

#[test]
fn test_a2a_event_type_task_submitted_serialize() {
    let json = serde_json::to_string(&A2AEventType::TaskSubmitted).unwrap();
    assert_eq!(json, "\"TASK_SUBMITTED\"");
}

#[test]
fn test_a2a_event_type_artifact_created_serialize() {
    let json = serde_json::to_string(&A2AEventType::ArtifactCreated).unwrap();
    assert_eq!(json, "\"ARTIFACT_CREATED\"");
}

#[test]
fn test_a2a_event_type_agent_message_serialize() {
    let json = serde_json::to_string(&A2AEventType::AgentMessage).unwrap();
    assert_eq!(json, "\"AGENT_MESSAGE\"");
}

#[test]
fn test_a2a_event_type_deserialize_task_status_update() {
    let t: A2AEventType = serde_json::from_str("\"TASK_STATUS_UPDATE\"").unwrap();
    assert!(matches!(t, A2AEventType::TaskStatusUpdate));
}

#[test]
fn test_a2a_event_type_deserialize_artifact_updated() {
    let t: A2AEventType = serde_json::from_str("\"ARTIFACT_UPDATED\"").unwrap();
    assert!(matches!(t, A2AEventType::ArtifactUpdated));
}

#[test]
fn test_a2a_event_type_deserialize_task_submitted() {
    let t: A2AEventType = serde_json::from_str("\"TASK_SUBMITTED\"").unwrap();
    assert!(matches!(t, A2AEventType::TaskSubmitted));
}

#[test]
fn test_a2a_event_type_equality() {
    assert_eq!(A2AEventType::TaskStatusUpdate, A2AEventType::TaskStatusUpdate);
    assert_eq!(A2AEventType::ArtifactUpdated, A2AEventType::ArtifactUpdated);
    assert_ne!(A2AEventType::TaskStatusUpdate, A2AEventType::ArtifactUpdated);
}

#[test]
fn test_a2a_event_type_copy() {
    let t = A2AEventType::TaskStatusUpdate;
    let copied = t;
    assert_eq!(t, copied);
}

#[test]
fn test_a2a_event_type_debug() {
    let t = A2AEventType::ArtifactUpdated;
    let debug_str = format!("{:?}", t);
    assert!(debug_str.contains("ArtifactUpdated"));
}

#[test]
fn test_a2a_event_type_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(A2AEventType::TaskStatusUpdate);
    set.insert(A2AEventType::ArtifactUpdated);
    set.insert(A2AEventType::TaskStatusUpdate); // duplicate

    assert_eq!(set.len(), 2);
}
