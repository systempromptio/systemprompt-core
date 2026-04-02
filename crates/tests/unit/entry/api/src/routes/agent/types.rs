use systemprompt_api::routes::agent::tasks::TaskFilterParams;
use systemprompt_api::routes::agent::artifacts::ArtifactQueryParams;

#[test]
fn test_task_filter_params_deserialize_empty() {
    let json = serde_json::json!({});
    let params: TaskFilterParams = serde_json::from_value(json).unwrap();
    assert!(params.status.is_none());
    assert!(params.limit.is_none());
}

#[test]
fn test_task_filter_params_deserialize_with_status() {
    let json = serde_json::json!({
        "status": "completed"
    });
    let params: TaskFilterParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.status.as_deref(), Some("completed"));
}

#[test]
fn test_task_filter_params_deserialize_with_limit() {
    let json = serde_json::json!({
        "limit": 50
    });
    let params: TaskFilterParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.limit, Some(50));
}

#[test]
fn test_task_filter_params_deserialize_all_fields() {
    let json = serde_json::json!({
        "status": "working",
        "limit": 10
    });
    let params: TaskFilterParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.status.as_deref(), Some("working"));
    assert_eq!(params.limit, Some(10));
}

#[test]
fn test_task_filter_params_debug_trait() {
    let json = serde_json::json!({
        "status": "submitted",
        "limit": 25
    });
    let params: TaskFilterParams = serde_json::from_value(json).unwrap();
    let debug = format!("{params:?}");
    assert!(debug.contains("TaskFilterParams"));
}

#[test]
fn test_artifact_query_params_deserialize_empty() {
    let json = serde_json::json!({});
    let params: ArtifactQueryParams = serde_json::from_value(json).unwrap();
    assert!(params.limit.is_none());
}

#[test]
fn test_artifact_query_params_deserialize_with_limit() {
    let json = serde_json::json!({
        "limit": 100
    });
    let params: ArtifactQueryParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.limit, Some(100));
}

#[test]
fn test_artifact_query_params_limit_zero() {
    let json = serde_json::json!({
        "limit": 0
    });
    let params: ArtifactQueryParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.limit, Some(0));
}

#[test]
fn test_artifact_query_params_debug_trait() {
    let json = serde_json::json!({
        "limit": 42
    });
    let params: ArtifactQueryParams = serde_json::from_value(json).unwrap();
    let debug = format!("{params:?}");
    assert!(debug.contains("ArtifactQueryParams"));
}

#[test]
fn test_artifact_query_params_copy_trait() {
    let json = serde_json::json!({
        "limit": 10
    });
    let params: ArtifactQueryParams = serde_json::from_value(json).unwrap();
    let copied = params;
    assert_eq!(copied.limit, Some(10));
    assert_eq!(params.limit, Some(10));
}
