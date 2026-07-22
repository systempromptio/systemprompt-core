use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::events::{
    A2AEvent, A2AEventBuilder, A2AEventType, AnalyticsEvent, AnalyticsEventBuilder,
    SessionStartedPayload,
};

#[test]
fn analytics_event_serializes_with_screaming_tag_and_flattened_payload() {
    let event = AnalyticsEventBuilder::session_started(SessionStartedPayload {
        session_id: SessionId::new("sess-1"),
        device_type: Some("desktop".to_owned()),
        browser: None,
        os: None,
        country: None,
        referrer_source: None,
        is_bot: false,
    });
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["type"], "SESSION_STARTED");
    assert_eq!(value["session_id"], "sess-1");
    assert_eq!(value["device_type"], "desktop");
    assert_eq!(value["is_bot"], false);
    assert!(value.get("payload").is_none());
}

#[test]
fn analytics_event_deserializes_flattened_shape() {
    let json = serde_json::json!({
        "type": "SESSION_ENDED",
        "timestamp": "2026-07-21T10:00:00Z",
        "session_id": "sess-2",
        "duration_ms": 1200,
        "page_count": 3,
        "request_count": 9
    });
    let event: AnalyticsEvent = serde_json::from_value(json).unwrap();
    match event {
        AnalyticsEvent::SessionEnded { payload, .. } => {
            assert_eq!(payload.session_id.as_str(), "sess-2");
            assert_eq!(payload.duration_ms, 1200);
            assert_eq!(payload.page_count, 3);
            assert_eq!(payload.request_count, 9);
        },
        other => panic!("expected SessionEnded, got {other:?}"),
    }
}

#[test]
fn analytics_timestamp_accessor_matches_every_variant() {
    let events = vec![
        AnalyticsEventBuilder::page_view(SessionId::new("s"), None, "/home".to_owned(), None, None),
        AnalyticsEventBuilder::engagement_update(
            SessionId::new("s"),
            "/home".to_owned(),
            50,
            1000,
            2,
        ),
        AnalyticsEventBuilder::realtime_stats(1, 1, 10, 5, 0),
        AnalyticsEventBuilder::heartbeat(),
    ];
    for event in events {
        let serialized = serde_json::to_value(&event).unwrap();
        let embedded = serialized["timestamp"]
            .as_str()
            .unwrap()
            .parse::<chrono::DateTime<chrono::Utc>>()
            .unwrap();
        assert_eq!(event.timestamp(), embedded);
    }
}

#[test]
fn a2a_event_type_maps_each_builder_variant() {
    let task = || TaskId::new("t1");
    let ctx = ContextId::generate();
    let ctx = move || ctx.clone();
    let cases = vec![
        (
            A2AEventBuilder::task_submitted(task(), ctx(), "agent".to_owned(), None),
            A2AEventType::TaskSubmitted,
        ),
        (
            A2AEventBuilder::task_status_update(task(), ctx(), TaskState::Working, None),
            A2AEventType::TaskStatusUpdate,
        ),
        (
            A2AEventBuilder::agent_message(task(), ctx(), MessageId::new("m1"), "hi".to_owned()),
            A2AEventType::AgentMessage,
        ),
        (
            A2AEventBuilder::input_required(task(), ctx(), "prompt".to_owned()),
            A2AEventType::InputRequired,
        ),
        (
            A2AEventBuilder::auth_required(task(), ctx(), "https://auth".to_owned()),
            A2AEventType::AuthRequired,
        ),
        (
            A2AEventBuilder::json_rpc_response(
                serde_json::json!(1),
                serde_json::json!({"ok": true}),
            ),
            A2AEventType::JsonRpcResponse,
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(event.event_type(), expected);
    }
}

#[test]
fn a2a_task_status_update_serializes_camel_case_and_skips_none_message() {
    let context_id = ContextId::generate();
    let event = A2AEventBuilder::task_status_update(
        TaskId::new("t1"),
        context_id.clone(),
        TaskState::Completed,
        None,
    );
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["type"], "TASK_STATUS_UPDATE");
    assert_eq!(value["taskId"], "t1");
    assert_eq!(value["contextId"], context_id.as_str());
    assert_eq!(value["state"], "TASK_STATE_COMPLETED");
    assert!(value.get("message").is_none());
}

#[test]
fn a2a_event_round_trips_through_json() {
    let event = A2AEventBuilder::task_submitted(
        TaskId::new("t2"),
        ContextId::generate(),
        "planner".to_owned(),
        Some(serde_json::json!({"q": "hello"})),
    );
    let json = serde_json::to_string(&event).unwrap();
    let back: A2AEvent = serde_json::from_str(&json).unwrap();
    match back {
        A2AEvent::TaskSubmitted { payload, .. } => {
            assert_eq!(payload.task_id.as_str(), "t2");
            assert_eq!(payload.agent_name, "planner");
            assert_eq!(payload.input, Some(serde_json::json!({"q": "hello"})));
        },
        other => panic!("expected TaskSubmitted, got {other:?}"),
    }
    assert_eq!(event.timestamp(), back_timestamp(&json));
}

fn back_timestamp(json: &str) -> chrono::DateTime<chrono::Utc> {
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    value["timestamp"]
        .as_str()
        .unwrap()
        .parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap()
}
