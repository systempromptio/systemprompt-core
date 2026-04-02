use serde_json::{Value, json};
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::{
    AgUiEvent, AgUiEventBuilder, AgUiEventType, CustomPayload, GenericCustomPayload,
    JsonPatchOperation, MessagesSnapshotPayload, RunErrorPayload, RunFinishedPayload,
    RunStartedPayload, StateDeltaBuilder, StateDeltaPayload, StateSnapshotPayload,
};
use systemprompt_models::agui::MessageRole;

#[test]
fn json_patch_add_factory() {
    let op = JsonPatchOperation::add("/foo", json!(42));
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["op"], "add");
    assert_eq!(json["path"], "/foo");
    assert_eq!(json["value"], 42);
}

#[test]
fn json_patch_remove_factory() {
    let op = JsonPatchOperation::remove("/bar");
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["op"], "remove");
    assert_eq!(json["path"], "/bar");
}

#[test]
fn json_patch_replace_factory() {
    let op = JsonPatchOperation::replace("/baz", json!("new"));
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["op"], "replace");
    assert_eq!(json["path"], "/baz");
    assert_eq!(json["value"], "new");
}

#[test]
fn json_patch_move_factory() {
    let op = JsonPatchOperation::move_op("/from", "/to");
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["op"], "move");
    assert_eq!(json["from"], "/from");
    assert_eq!(json["path"], "/to");
}

#[test]
fn json_patch_copy_factory() {
    let op = JsonPatchOperation::copy("/src", "/dst");
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["op"], "copy");
    assert_eq!(json["from"], "/src");
    assert_eq!(json["path"], "/dst");
}

#[test]
fn json_patch_test_factory() {
    let op = JsonPatchOperation::test("/check", json!(true));
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["op"], "test");
    assert_eq!(json["path"], "/check");
    assert_eq!(json["value"], true);
}

#[test]
fn json_patch_add_serde_roundtrip() {
    let op = JsonPatchOperation::add("/path", json!({"key": "value"}));
    let serialized = serde_json::to_string(&op).unwrap();
    let deserialized: JsonPatchOperation = serde_json::from_str(&serialized).unwrap();
    let json = serde_json::to_value(&deserialized).unwrap();
    assert_eq!(json["op"], "add");
    assert_eq!(json["path"], "/path");
}

#[test]
fn json_patch_remove_serde_roundtrip() {
    let op = JsonPatchOperation::remove("/items/0");
    let serialized = serde_json::to_string(&op).unwrap();
    let deserialized: JsonPatchOperation = serde_json::from_str(&serialized).unwrap();
    let json = serde_json::to_value(&deserialized).unwrap();
    assert_eq!(json["op"], "remove");
}

#[test]
fn json_patch_replace_with_null_value() {
    let op = JsonPatchOperation::replace("/field", Value::Null);
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["value"], Value::Null);
}

#[test]
fn json_patch_add_with_array_value() {
    let op = JsonPatchOperation::add("/items", json!([1, 2, 3]));
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["value"], json!([1, 2, 3]));
}

#[test]
fn json_patch_add_with_nested_object() {
    let op = JsonPatchOperation::add("/deep", json!({"a": {"b": {"c": 1}}}));
    let json = serde_json::to_value(&op).unwrap();
    assert_eq!(json["value"]["a"]["b"]["c"], 1);
}

#[test]
fn state_delta_builder_empty() {
    let ops = StateDeltaBuilder::new().build();
    assert!(ops.is_empty());
}

#[test]
fn state_delta_builder_single_add() {
    let ops = StateDeltaBuilder::new()
        .add("/count", json!(0))
        .build();
    assert_eq!(ops.len(), 1);
    let json = serde_json::to_value(&ops[0]).unwrap();
    assert_eq!(json["op"], "add");
}

#[test]
fn state_delta_builder_single_replace() {
    let ops = StateDeltaBuilder::new()
        .replace("/count", json!(1))
        .build();
    assert_eq!(ops.len(), 1);
    let json = serde_json::to_value(&ops[0]).unwrap();
    assert_eq!(json["op"], "replace");
}

#[test]
fn state_delta_builder_single_remove() {
    let ops = StateDeltaBuilder::new()
        .remove("/old")
        .build();
    assert_eq!(ops.len(), 1);
    let json = serde_json::to_value(&ops[0]).unwrap();
    assert_eq!(json["op"], "remove");
}

#[test]
fn state_delta_builder_chained_operations() {
    let ops = StateDeltaBuilder::new()
        .add("/a", json!(1))
        .replace("/b", json!(2))
        .remove("/c")
        .build();
    assert_eq!(ops.len(), 3);
}

#[test]
fn state_delta_builder_default() {
    let builder = StateDeltaBuilder::default();
    let ops = builder.build();
    assert!(ops.is_empty());
}

#[test]
fn event_type_as_str_run_started() {
    assert_eq!(AgUiEventType::RunStarted.as_str(), "RUN_STARTED");
}

#[test]
fn event_type_as_str_run_finished() {
    assert_eq!(AgUiEventType::RunFinished.as_str(), "RUN_FINISHED");
}

#[test]
fn event_type_as_str_run_error() {
    assert_eq!(AgUiEventType::RunError.as_str(), "RUN_ERROR");
}

#[test]
fn event_type_as_str_text_message_start() {
    assert_eq!(AgUiEventType::TextMessageStart.as_str(), "TEXT_MESSAGE_START");
}

#[test]
fn event_type_as_str_tool_call_start() {
    assert_eq!(AgUiEventType::ToolCallStart.as_str(), "TOOL_CALL_START");
}

#[test]
fn event_type_as_str_state_snapshot() {
    assert_eq!(AgUiEventType::StateSnapshot.as_str(), "STATE_SNAPSHOT");
}

#[test]
fn event_type_as_str_state_delta() {
    assert_eq!(AgUiEventType::StateDelta.as_str(), "STATE_DELTA");
}

#[test]
fn event_type_as_str_custom() {
    assert_eq!(AgUiEventType::Custom.as_str(), "CUSTOM");
}

#[test]
fn event_type_serde_roundtrip() {
    let event_type = AgUiEventType::TextMessageContent;
    let json = serde_json::to_string(&event_type).unwrap();
    let deserialized: AgUiEventType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, event_type);
}

#[test]
fn event_type_all_variants_as_str() {
    let pairs = [
        (AgUiEventType::RunStarted, "RUN_STARTED"),
        (AgUiEventType::RunFinished, "RUN_FINISHED"),
        (AgUiEventType::RunError, "RUN_ERROR"),
        (AgUiEventType::StepStarted, "STEP_STARTED"),
        (AgUiEventType::StepFinished, "STEP_FINISHED"),
        (AgUiEventType::TextMessageStart, "TEXT_MESSAGE_START"),
        (AgUiEventType::TextMessageContent, "TEXT_MESSAGE_CONTENT"),
        (AgUiEventType::TextMessageEnd, "TEXT_MESSAGE_END"),
        (AgUiEventType::ToolCallStart, "TOOL_CALL_START"),
        (AgUiEventType::ToolCallArgs, "TOOL_CALL_ARGS"),
        (AgUiEventType::ToolCallEnd, "TOOL_CALL_END"),
        (AgUiEventType::ToolCallResult, "TOOL_CALL_RESULT"),
        (AgUiEventType::StateSnapshot, "STATE_SNAPSHOT"),
        (AgUiEventType::StateDelta, "STATE_DELTA"),
        (AgUiEventType::MessagesSnapshot, "MESSAGES_SNAPSHOT"),
        (AgUiEventType::Custom, "CUSTOM"),
    ];
    for (variant, expected) in pairs {
        assert_eq!(variant.as_str(), expected);
    }
}

#[test]
fn builder_run_started() {
    let ctx = ContextId::new("ctx-1");
    let task = TaskId::new("task-1");
    let event = AgUiEventBuilder::run_started(ctx, task, Some(json!({"prompt": "hello"})));
    assert_eq!(event.event_type(), AgUiEventType::RunStarted);
}

#[test]
fn builder_run_started_no_input() {
    let ctx = ContextId::new("ctx-1");
    let task = TaskId::new("task-1");
    let event = AgUiEventBuilder::run_started(ctx, task, None);
    assert_eq!(event.event_type(), AgUiEventType::RunStarted);
}

#[test]
fn builder_run_finished() {
    let ctx = ContextId::new("ctx-1");
    let task = TaskId::new("task-1");
    let event = AgUiEventBuilder::run_finished(ctx, task, Some(json!("done")));
    assert_eq!(event.event_type(), AgUiEventType::RunFinished);
}

#[test]
fn builder_run_error() {
    let event = AgUiEventBuilder::run_error("something failed".to_string(), Some("E001".to_string()));
    assert_eq!(event.event_type(), AgUiEventType::RunError);
}

#[test]
fn builder_run_error_no_code() {
    let event = AgUiEventBuilder::run_error("failure".to_string(), None);
    assert_eq!(event.event_type(), AgUiEventType::RunError);
}

#[test]
fn builder_step_started() {
    let event = AgUiEventBuilder::step_started("planning");
    assert_eq!(event.event_type(), AgUiEventType::StepStarted);
}

#[test]
fn builder_step_finished() {
    let event = AgUiEventBuilder::step_finished("planning");
    assert_eq!(event.event_type(), AgUiEventType::StepFinished);
}

#[test]
fn builder_text_message_start() {
    let event = AgUiEventBuilder::text_message_start("msg-1", MessageRole::Assistant);
    assert_eq!(event.event_type(), AgUiEventType::TextMessageStart);
}

#[test]
fn builder_text_message_content() {
    let event = AgUiEventBuilder::text_message_content("msg-1", "Hello ");
    assert_eq!(event.event_type(), AgUiEventType::TextMessageContent);
}

#[test]
fn builder_text_message_end() {
    let event = AgUiEventBuilder::text_message_end("msg-1");
    assert_eq!(event.event_type(), AgUiEventType::TextMessageEnd);
}

#[test]
fn builder_tool_call_start() {
    let event = AgUiEventBuilder::tool_call_start("tc-1", "search", Some("msg-1".to_string()));
    assert_eq!(event.event_type(), AgUiEventType::ToolCallStart);
}

#[test]
fn builder_tool_call_start_no_parent() {
    let event = AgUiEventBuilder::tool_call_start("tc-1", "search", None);
    assert_eq!(event.event_type(), AgUiEventType::ToolCallStart);
}

#[test]
fn builder_tool_call_args() {
    let event = AgUiEventBuilder::tool_call_args("tc-1", r#"{"query":"rust"}"#);
    assert_eq!(event.event_type(), AgUiEventType::ToolCallArgs);
}

#[test]
fn builder_tool_call_end() {
    let event = AgUiEventBuilder::tool_call_end("tc-1");
    assert_eq!(event.event_type(), AgUiEventType::ToolCallEnd);
}

#[test]
fn builder_tool_call_result() {
    let event = AgUiEventBuilder::tool_call_result("msg-2", "tc-1", json!({"results": []}));
    assert_eq!(event.event_type(), AgUiEventType::ToolCallResult);
}

#[test]
fn builder_state_snapshot() {
    let event = AgUiEventBuilder::state_snapshot(json!({"count": 0}));
    assert_eq!(event.event_type(), AgUiEventType::StateSnapshot);
}

#[test]
fn builder_state_delta() {
    let ops = vec![JsonPatchOperation::add("/count", json!(1))];
    let event = AgUiEventBuilder::state_delta(ops);
    assert_eq!(event.event_type(), AgUiEventType::StateDelta);
}

#[test]
fn builder_state_delta_empty_ops() {
    let event = AgUiEventBuilder::state_delta(vec![]);
    assert_eq!(event.event_type(), AgUiEventType::StateDelta);
}

#[test]
fn builder_messages_snapshot() {
    let event = AgUiEventBuilder::messages_snapshot(vec![json!({"role": "user", "content": "hi"})]);
    assert_eq!(event.event_type(), AgUiEventType::MessagesSnapshot);
}

#[test]
fn builder_messages_snapshot_empty() {
    let event = AgUiEventBuilder::messages_snapshot(vec![]);
    assert_eq!(event.event_type(), AgUiEventType::MessagesSnapshot);
}

#[test]
fn builder_custom_generic() {
    let payload = CustomPayload::Generic(GenericCustomPayload {
        name: "my_event".to_string(),
        value: json!({"data": 123}),
    });
    let event = AgUiEventBuilder::custom(payload);
    assert_eq!(event.event_type(), AgUiEventType::Custom);
}

#[test]
fn event_timestamp_is_populated() {
    let event = AgUiEventBuilder::step_started("test");
    let ts = event.timestamp();
    assert!(ts.timestamp() > 0);
}

#[test]
fn event_serde_roundtrip_run_started() {
    let ctx = ContextId::new("ctx-1");
    let task = TaskId::new("task-1");
    let event = AgUiEventBuilder::run_started(ctx, task, Some(json!({"key": "val"})));
    let json_str = serde_json::to_string(&event).unwrap();
    let deserialized: AgUiEvent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.event_type(), AgUiEventType::RunStarted);
}

#[test]
fn event_serde_roundtrip_text_message_content() {
    let event = AgUiEventBuilder::text_message_content("msg-1", "hello world");
    let json_str = serde_json::to_string(&event).unwrap();
    let deserialized: AgUiEvent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.event_type(), AgUiEventType::TextMessageContent);
}

#[test]
fn event_serde_roundtrip_state_delta() {
    let ops = StateDeltaBuilder::new()
        .add("/a", json!(1))
        .replace("/b", json!("x"))
        .remove("/c")
        .build();
    let event = AgUiEventBuilder::state_delta(ops);
    let json_str = serde_json::to_string(&event).unwrap();
    let deserialized: AgUiEvent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.event_type(), AgUiEventType::StateDelta);
}

#[test]
fn event_serde_roundtrip_state_snapshot() {
    let event = AgUiEventBuilder::state_snapshot(json!({}));
    let json_str = serde_json::to_string(&event).unwrap();
    let deserialized: AgUiEvent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.event_type(), AgUiEventType::StateSnapshot);
}

#[test]
fn event_serde_run_started_has_type_field() {
    let ctx = ContextId::new("ctx-1");
    let task = TaskId::new("task-1");
    let event = AgUiEventBuilder::run_started(ctx, task, None);
    let json_val = serde_json::to_value(&event).unwrap();
    assert_eq!(json_val["type"], "RUN_STARTED");
}

#[test]
fn event_serde_run_error_has_type_field() {
    let event = AgUiEventBuilder::run_error("err".to_string(), None);
    let json_val = serde_json::to_value(&event).unwrap();
    assert_eq!(json_val["type"], "RUN_ERROR");
}

#[test]
fn message_role_serde_roundtrip() {
    for role in [MessageRole::User, MessageRole::Assistant, MessageRole::System, MessageRole::Tool] {
        let json = serde_json::to_string(&role).unwrap();
        let deserialized: MessageRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, role);
    }
}

#[test]
fn message_role_serializes_lowercase() {
    let json = serde_json::to_string(&MessageRole::Assistant).unwrap();
    assert_eq!(json, "\"assistant\"");
}

#[test]
fn run_started_payload_serde() {
    let payload = RunStartedPayload {
        thread_id: ContextId::new("ctx"),
        run_id: TaskId::new("task"),
        input: Some(json!("hi")),
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: RunStartedPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.thread_id.as_str(), "ctx");
    assert_eq!(deserialized.run_id.as_str(), "task");
}

#[test]
fn run_finished_payload_serde() {
    let payload = RunFinishedPayload {
        thread_id: ContextId::new("ctx"),
        run_id: TaskId::new("task"),
        result: None,
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: RunFinishedPayload = serde_json::from_str(&json_str).unwrap();
    assert!(deserialized.result.is_none());
}

#[test]
fn run_error_payload_serde() {
    let payload = RunErrorPayload {
        message: "bad request".to_string(),
        code: Some("400".to_string()),
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: RunErrorPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.message, "bad request");
    assert_eq!(deserialized.code.as_deref(), Some("400"));
}

#[test]
fn state_snapshot_payload_serde() {
    let payload = StateSnapshotPayload {
        snapshot: json!({"key": "value"}),
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: StateSnapshotPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.snapshot["key"], "value");
}

#[test]
fn state_delta_payload_serde() {
    let payload = StateDeltaPayload {
        delta: vec![JsonPatchOperation::add("/x", json!(1))],
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: StateDeltaPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.delta.len(), 1);
}

#[test]
fn messages_snapshot_payload_serde() {
    let payload = MessagesSnapshotPayload {
        messages: vec![json!({"role": "user"}), json!({"role": "assistant"})],
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: MessagesSnapshotPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.messages.len(), 2);
}

#[test]
fn messages_snapshot_payload_empty() {
    let payload = MessagesSnapshotPayload { messages: vec![] };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: MessagesSnapshotPayload = serde_json::from_str(&json_str).unwrap();
    assert!(deserialized.messages.is_empty());
}

#[test]
fn state_snapshot_payload_empty_object() {
    let payload = StateSnapshotPayload { snapshot: json!({}) };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: StateSnapshotPayload = serde_json::from_str(&json_str).unwrap();
    assert!(deserialized.snapshot.as_object().unwrap().is_empty());
}

#[test]
fn state_snapshot_payload_nested_structure() {
    let payload = StateSnapshotPayload {
        snapshot: json!({"a": {"b": {"c": [1, 2, 3]}}}),
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: StateSnapshotPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.snapshot["a"]["b"]["c"][1], 2);
}

#[test]
fn generic_custom_payload_serde() {
    let payload = GenericCustomPayload {
        name: "custom_event".to_string(),
        value: json!({"foo": "bar"}),
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: GenericCustomPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.name, "custom_event");
    assert_eq!(deserialized.value["foo"], "bar");
}

#[test]
fn generic_custom_payload_empty_value() {
    let payload = GenericCustomPayload {
        name: "empty".to_string(),
        value: json!(null),
    };
    let json_str = serde_json::to_string(&payload).unwrap();
    let deserialized: GenericCustomPayload = serde_json::from_str(&json_str).unwrap();
    assert!(deserialized.value.is_null());
}
