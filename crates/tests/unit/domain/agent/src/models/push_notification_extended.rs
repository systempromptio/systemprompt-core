use systemprompt_agent::models::a2a::protocol::{
    DeleteTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigRequest,
    ListTaskPushNotificationConfigRequest, ListTaskPushNotificationConfigResponse,
    PushNotificationConfig, PushNotificationNotSupportedError, SetTaskPushNotificationConfigRequest,
    SetTaskPushNotificationConfigResponse, TaskPushNotificationConfig,
    TaskResubscriptionRequest, TaskResubscriptionResponse,
};
use systemprompt_identifiers::TaskId;

fn minimal_config() -> PushNotificationConfig {
    PushNotificationConfig {
        url: "https://hook.example.com/notify".to_string(),
        token: None,
        authentication: None,
        endpoint: String::new(),
        headers: None,
    }
}

#[test]
fn push_notification_config_serde_roundtrip() {
    let config = PushNotificationConfig {
        url: "https://notify.example.com".to_string(),
        token: Some("Bearer abc123".to_string()),
        authentication: None,
        endpoint: String::new(),
        headers: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: PushNotificationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.url, "https://notify.example.com");
    assert_eq!(de.token, Some("Bearer abc123".to_string()));
}

#[test]
fn push_notification_config_clone_eq() {
    let config = minimal_config();
    let cloned = config.clone();
    assert_eq!(config, cloned);
}

#[test]
fn push_notification_not_supported_error_serde() {
    let err = PushNotificationNotSupportedError {
        message: "This agent does not support push notifications".to_string(),
    };
    let json = serde_json::to_string(&err).unwrap();
    let de: PushNotificationNotSupportedError = serde_json::from_str(&json).unwrap();
    assert_eq!(de.message, err.message);
}

#[test]
fn push_notification_not_supported_error_clone_eq() {
    let err = PushNotificationNotSupportedError {
        message: "unsupported".to_string(),
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn task_push_notification_config_serde_roundtrip() {
    let config = TaskPushNotificationConfig {
        id: TaskId::new("task-pn-1"),
        push_notification_config: minimal_config(),
    };
    let json = serde_json::to_string(&config).unwrap();
    let de: TaskPushNotificationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id.as_str(), "task-pn-1");
}

#[test]
fn set_push_notification_config_request_serde() {
    let req = SetTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-set"),
        config: minimal_config(),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: SetTaskPushNotificationConfigRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-set");
}

#[test]
fn set_push_notification_config_response_serde() {
    let resp = SetTaskPushNotificationConfigResponse {
        success: true,
        message: Some("Configuration updated".to_string()),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let de: SetTaskPushNotificationConfigResponse = serde_json::from_str(&json).unwrap();
    assert!(de.success);
    assert_eq!(de.message, Some("Configuration updated".to_string()));
}

#[test]
fn get_push_notification_config_request_serde() {
    let req = GetTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-get"),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: GetTaskPushNotificationConfigRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-get");
}

#[test]
fn delete_push_notification_config_request_serde() {
    let req = DeleteTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-del"),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: DeleteTaskPushNotificationConfigRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-del");
}

#[test]
fn list_push_notification_config_request_serde() {
    let req = ListTaskPushNotificationConfigRequest {
        task_id: TaskId::new("task-list"),
        limit: Some(10),
        offset: Some(0),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: ListTaskPushNotificationConfigRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-list");
    assert_eq!(de.limit, Some(10));
    assert_eq!(de.offset, Some(0));
}

#[test]
fn list_push_notification_config_response_serde() {
    let resp = ListTaskPushNotificationConfigResponse {
        configs: vec![minimal_config()],
        total: 1,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let de: ListTaskPushNotificationConfigResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(de.total, 1);
    assert_eq!(de.configs.len(), 1);
}

#[test]
fn task_resubscription_request_serde() {
    let req = TaskResubscriptionRequest {
        task_id: TaskId::new("task-resub"),
        config: minimal_config(),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: TaskResubscriptionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.task_id.as_str(), "task-resub");
}

#[test]
fn task_resubscription_response_success() {
    let resp = TaskResubscriptionResponse {
        success: true,
        message: None,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let de: TaskResubscriptionResponse = serde_json::from_str(&json).unwrap();
    assert!(de.success);
    assert!(de.message.is_none());
}

#[test]
fn task_resubscription_response_clone_eq() {
    let resp = TaskResubscriptionResponse {
        success: false,
        message: Some("error".to_string()),
    };
    let cloned = resp.clone();
    assert_eq!(resp, cloned);
}

#[test]
fn push_notification_config_debug() {
    let config = minimal_config();
    let dbg = format!("{:?}", config);
    assert!(dbg.contains("PushNotificationConfig"));
}
