use systemprompt_agent::models::a2a::TaskState;
use systemprompt_agent::models::a2a::protocol::{
    MessageSendConfiguration, PushNotificationConfig, TaskNotCancelableError, TaskNotFoundError,
    UnsupportedOperationError,
};
use systemprompt_identifiers::TaskId;

#[test]
fn message_send_configuration_serialize_camel_case() {
    let cfg = MessageSendConfiguration {
        accepted_output_modes: Some(vec!["text/plain".to_string()]),
        history_length: Some(50),
        push_notification_config: Some(PushNotificationConfig {
            endpoint: String::new(),
            headers: None,
            url: "https://example.com/cb".to_string(),
            token: None,
            authentication: None,
        }),
        blocking: Some(true),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    assert!(json.contains("acceptedOutputModes"));
    assert!(json.contains("historyLength"));
    assert!(json.contains("pushNotificationConfig"));
    assert!(json.contains("blocking"));
}

#[test]
fn message_send_configuration_deserialize_camel_case() {
    let json = r#"{
        "acceptedOutputModes": ["text/plain"],
        "historyLength": 10,
        "pushNotificationConfig": null,
        "blocking": false
    }"#;
    let cfg: MessageSendConfiguration = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.history_length, Some(10));
    assert_eq!(cfg.blocking, Some(false));
    assert!(cfg.push_notification_config.is_none());
}

#[test]
fn message_send_configuration_optional_fields_none() {
    let cfg = MessageSendConfiguration {
        accepted_output_modes: None,
        history_length: None,
        push_notification_config: None,
        blocking: None,
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let back: MessageSendConfiguration = serde_json::from_str(&json).unwrap();
    assert!(back.accepted_output_modes.is_none());
    assert!(back.history_length.is_none());
    assert!(back.blocking.is_none());
}

#[test]
fn task_not_found_error_round_trip() {
    let err = TaskNotFoundError {
        task_id: TaskId::new("t-missing"),
        message: "task not found".to_string(),
        code: -32001,
        data: serde_json::json!({"hint": "check id"}),
    };
    let json = serde_json::to_string(&err).unwrap();
    let back: TaskNotFoundError = serde_json::from_str(&json).unwrap();
    assert_eq!(back.task_id, err.task_id);
    assert_eq!(back.code, -32001);
    assert_eq!(back.message, "task not found");
}

#[test]
fn task_not_cancelable_error_round_trip() {
    let err = TaskNotCancelableError {
        task_id: TaskId::new("t-done"),
        state: TaskState::Completed,
        message: "task already completed".to_string(),
        code: -32002,
        data: serde_json::Value::Null,
    };
    let json = serde_json::to_string(&err).unwrap();
    let back: TaskNotCancelableError = serde_json::from_str(&json).unwrap();
    assert_eq!(back.task_id, err.task_id);
    assert_eq!(back.state, TaskState::Completed);
}

#[test]
fn unsupported_operation_error_round_trip() {
    let err = UnsupportedOperationError {
        operation: "magic".to_string(),
        message: "magic not implemented".to_string(),
        code: -32004,
        data: serde_json::json!(null),
    };
    let json = serde_json::to_string(&err).unwrap();
    let back: UnsupportedOperationError = serde_json::from_str(&json).unwrap();
    assert_eq!(back.operation, "magic");
    assert_eq!(back.code, -32004);
}

#[test]
fn task_not_found_error_clone_eq() {
    let err = TaskNotFoundError {
        task_id: TaskId::new("t"),
        message: "m".to_string(),
        code: 1,
        data: serde_json::Value::Null,
    };
    assert_eq!(err.clone(), err);
}

#[test]
fn task_not_cancelable_error_debug() {
    let err = TaskNotCancelableError {
        task_id: TaskId::new("t"),
        state: TaskState::Working,
        message: "m".to_string(),
        code: 2,
        data: serde_json::Value::Null,
    };
    assert!(format!("{:?}", err).contains("TaskNotCancelableError"));
}
