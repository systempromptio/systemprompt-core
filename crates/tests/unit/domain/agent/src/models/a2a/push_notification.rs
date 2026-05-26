use systemprompt_agent::models::a2a::protocol::{
    DeleteTaskPushNotificationConfigParams, DeleteTaskPushNotificationConfigRequest,
    DeleteTaskPushNotificationConfigResponse, GetTaskPushNotificationConfigParams,
    GetTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigResponse,
    ListTaskPushNotificationConfigRequest, ListTaskPushNotificationConfigResponse,
    PushNotificationConfig, PushNotificationNotSupportedError,
    SetTaskPushNotificationConfigRequest, SetTaskPushNotificationConfigResponse,
    TaskPushNotificationConfig, TaskResubscriptionRequest, TaskResubscriptionResponse,
};
use systemprompt_identifiers::TaskId;

fn sample_config() -> PushNotificationConfig {
    PushNotificationConfig {
        endpoint: "endpoint-1".to_string(),
        headers: Some(serde_json::Map::new()),
        url: "https://example.com/hook".to_string(),
        token: Some("secret".to_string()),
        authentication: None,
    }
}

#[test]
fn push_notification_config_serialize_roundtrip() {
    let cfg = sample_config();
    let json = serde_json::to_string(&cfg).unwrap();
    let back: PushNotificationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.url, cfg.url);
    assert_eq!(back.endpoint, cfg.endpoint);
    assert_eq!(back.token, cfg.token);
}

#[test]
fn push_notification_config_skips_empty_endpoint() {
    let cfg = PushNotificationConfig {
        endpoint: String::new(),
        headers: None,
        url: "https://example.com".to_string(),
        token: None,
        authentication: None,
    };
    let json = serde_json::to_string(&cfg).unwrap();
    assert!(!json.contains("endpoint"));
}

#[test]
fn push_notification_config_default_endpoint_via_deserialize() {
    let json = r#"{"url": "https://example.com"}"#;
    let cfg: PushNotificationConfig = serde_json::from_str(json).unwrap();
    assert!(cfg.endpoint.is_empty());
    assert!(cfg.token.is_none());
}

#[test]
fn push_notification_config_clone_eq() {
    let cfg = sample_config();
    let cloned = cfg.clone();
    assert_eq!(cloned, cfg);
}

#[test]
fn push_notification_not_supported_error_serialize() {
    let err = PushNotificationNotSupportedError {
        message: "nope".to_string(),
    };
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("nope"));
}

#[test]
fn task_push_notification_config_round_trip() {
    let value = TaskPushNotificationConfig {
        id: TaskId::new("task-1"),
        push_notification_config: sample_config(),
    };
    let json = serde_json::to_string(&value).unwrap();
    let back: TaskPushNotificationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, value.id);
}

#[test]
fn set_task_push_notification_config_round_trip() {
    let req = SetTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-set"),
        config: sample_config(),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: SetTaskPushNotificationConfigRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.task_id, req.task_id);

    let resp = SetTaskPushNotificationConfigResponse {
        success: true,
        message: Some("ok".to_string()),
    };
    let json2 = serde_json::to_string(&resp).unwrap();
    let back2: SetTaskPushNotificationConfigResponse = serde_json::from_str(&json2).unwrap();
    assert!(back2.success);
}

#[test]
fn get_task_push_notification_config_round_trip() {
    let req = GetTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-get"),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: GetTaskPushNotificationConfigRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.task_id, req.task_id);

    let resp = GetTaskPushNotificationConfigResponse {
        config: Some(sample_config()),
    };
    let json2 = serde_json::to_string(&resp).unwrap();
    let back2: GetTaskPushNotificationConfigResponse = serde_json::from_str(&json2).unwrap();
    assert!(back2.config.is_some());

    let params = GetTaskPushNotificationConfigParams {
        id: TaskId::new("task-params"),
    };
    let json3 = serde_json::to_string(&params).unwrap();
    assert!(json3.contains("task-params"));
}

#[test]
fn delete_task_push_notification_config_round_trip() {
    let req = DeleteTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-del"),
    };
    let back: DeleteTaskPushNotificationConfigRequest =
        serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    assert_eq!(back.task_id, req.task_id);

    let resp = DeleteTaskPushNotificationConfigResponse {
        success: false,
        message: None,
    };
    let back2: DeleteTaskPushNotificationConfigResponse =
        serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
    assert!(!back2.success);

    let params = DeleteTaskPushNotificationConfigParams {
        id: TaskId::new("p"),
    };
    let json3 = serde_json::to_string(&params).unwrap();
    assert!(json3.contains("\"p\""));
}

#[test]
fn list_task_push_notification_config_round_trip() {
    let req = ListTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-list"),
        limit: Some(10),
        offset: Some(20),
    };
    let back: ListTaskPushNotificationConfigRequest =
        serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    assert_eq!(back.limit, Some(10));
    assert_eq!(back.offset, Some(20));

    let resp = ListTaskPushNotificationConfigResponse {
        configs: vec![sample_config(), sample_config()],
        total: 2,
    };
    let back2: ListTaskPushNotificationConfigResponse =
        serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
    assert_eq!(back2.configs.len(), 2);
    assert_eq!(back2.total, 2);
}

#[test]
fn task_resubscription_request_round_trip() {
    let req = TaskResubscriptionRequest {
        task_id: TaskId::new("resub"),
        config: sample_config(),
    };
    let back: TaskResubscriptionRequest =
        serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    assert_eq!(back.task_id, req.task_id);

    let resp = TaskResubscriptionResponse {
        success: true,
        message: Some("subscribed".to_string()),
    };
    let back2: TaskResubscriptionResponse =
        serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
    assert!(back2.success);
}

#[test]
fn push_notification_types_debug_present() {
    assert!(format!("{:?}", sample_config()).contains("PushNotificationConfig"));
    assert!(
        format!(
            "{:?}",
            PushNotificationNotSupportedError {
                message: "x".to_string()
            }
        )
        .contains("PushNotificationNotSupportedError")
    );
}
