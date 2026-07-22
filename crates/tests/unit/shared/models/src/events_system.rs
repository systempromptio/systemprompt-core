//! Unit tests for `SystemEvent` builders, payload serialization, and the
//! `ContextEvent` envelope.

use chrono::Utc;
use systemprompt_identifiers::{ConnectionId, ContextId};
use systemprompt_models::events::payloads::system::ContextSummary;
use systemprompt_models::events::{A2AEventType, ContextEvent, SystemEvent, SystemEventBuilder};
use systemprompt_traits::ContextWithStats;

const CTX: &str = "00000000-0000-4000-8000-00000000e001";

#[test]
fn context_created_serializes_with_screaming_tag_and_flattened_payload() {
    let event = SystemEventBuilder::context_created(ContextId::new(CTX), "My Context".to_owned());

    assert_eq!(
        event.event_type(),
        systemprompt_models::SystemEventType::ContextCreated
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "CONTEXT_CREATED");
    assert_eq!(json["contextId"], CTX);
    assert_eq!(json["name"], "My Context");
}

#[test]
fn context_updated_omits_absent_name() {
    let event = SystemEventBuilder::context_updated(ContextId::new(CTX), None);

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "CONTEXT_UPDATED");
    assert!(json.get("name").is_none());

    let named = SystemEventBuilder::context_updated(ContextId::new(CTX), Some("N".to_owned()));
    assert_eq!(serde_json::to_value(&named).unwrap()["name"], "N");
}

#[test]
fn context_deleted_and_heartbeat_round_trip() {
    let deleted = SystemEventBuilder::context_deleted(ContextId::new(CTX));
    let json = serde_json::to_string(&deleted).unwrap();
    let back: SystemEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, SystemEvent::ContextDeleted { .. }));

    let hb = SystemEventBuilder::heartbeat();
    assert_eq!(
        hb.event_type(),
        systemprompt_models::SystemEventType::Heartbeat
    );
    assert!(hb.timestamp() <= Utc::now());
}

#[test]
fn connected_event_carries_connection_id() {
    let event = SystemEventBuilder::connected(ConnectionId::new("conn-1"));
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "CONNECTED");
    assert_eq!(json["connectionId"], "conn-1");
}

#[test]
fn contexts_snapshot_serializes_summaries() {
    let stats = ContextWithStats {
        context_id: ContextId::new(CTX),
        user_id: systemprompt_identifiers::UserId::new("user-1"),
        name: "ctx".to_owned(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        message_count: 3,
        task_count: 2,
        last_message_at: None,
    };
    let by_ref: ContextSummary = (&stats).into();
    let by_val: ContextSummary = stats.into();
    assert_eq!(by_ref.context_id.as_str(), CTX);
    assert_eq!(by_val.message_count, 3);
    assert_eq!(by_val.task_count, 2);

    let event = SystemEventBuilder::contexts_snapshot(vec![by_val]);
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "CONTEXTS_SNAPSHOT");
    assert_eq!(json["contexts"][0]["name"], "ctx");
}

#[test]
fn context_event_wraps_system_event_with_protocol_tag() {
    let event: ContextEvent = SystemEventBuilder::heartbeat().into();
    let ts = event.timestamp();
    assert!(ts <= Utc::now());

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["protocol"], "system");
    assert_eq!(json["event"]["type"], "HEARTBEAT");
}

#[test]
fn a2a_event_type_as_str_matches_serde_tags() {
    for ty in [
        A2AEventType::TaskSubmitted,
        A2AEventType::TaskStatusUpdate,
        A2AEventType::ArtifactCreated,
        A2AEventType::ArtifactUpdated,
        A2AEventType::AgentMessage,
        A2AEventType::InputRequired,
        A2AEventType::AuthRequired,
        A2AEventType::JsonRpcResponse,
        A2AEventType::JsonRpcError,
    ] {
        let serde_tag = serde_json::to_value(ty).unwrap();
        assert_eq!(serde_tag, ty.as_str(), "as_str must match the wire tag");
    }
}
