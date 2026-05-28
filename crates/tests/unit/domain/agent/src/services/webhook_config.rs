use std::time::Duration;
use systemprompt_agent::services::external_integrations::{
    RetryPolicy, WebhookConfig, WebhookDeliveryResult,
};

#[test]
fn webhook_config_default_has_30s_timeout() {
    let cfg = WebhookConfig::default();
    assert!(cfg.secret.is_none());
    assert!(cfg.headers.is_empty());
    assert_eq!(cfg.timeout, Some(Duration::from_secs(30)));
}

#[test]
fn webhook_config_clone() {
    let mut cfg = WebhookConfig::default();
    cfg.secret = Some("hush".to_string());
    cfg.headers.insert("X-Trace".to_string(), "abc".to_string());
    let cloned = cfg.clone();
    assert_eq!(cloned.secret.as_deref(), Some("hush"));
    assert_eq!(
        cloned.headers.get("X-Trace").map(|s| s.as_str()),
        Some("abc")
    );
}

#[test]
fn webhook_config_debug_includes_struct_name() {
    let cfg = WebhookConfig::default();
    assert!(format!("{:?}", cfg).contains("WebhookConfig"));
}

#[test]
fn retry_policy_default_values() {
    let p = RetryPolicy::default();
    assert_eq!(p.max_retries, 3);
    assert_eq!(p.initial_delay_ms, 1000);
    assert_eq!(p.max_delay_ms, 30000);
    assert!((p.backoff_factor - 2.0).abs() < f64::EPSILON);
}

#[test]
fn retry_policy_clone_and_copy() {
    let p = RetryPolicy::default();
    let copied = p;
    let cloned = copied.clone();
    assert_eq!(cloned.max_retries, copied.max_retries);
}

#[test]
fn retry_policy_debug() {
    let p = RetryPolicy::default();
    assert!(format!("{:?}", p).contains("RetryPolicy"));
}

#[test]
fn webhook_delivery_result_success_fields() {
    let res = WebhookDeliveryResult {
        success: true,
        status_code: 200,
        response_body: "ok".to_string(),
        response_headers: std::collections::HashMap::new(),
        duration_ms: 42,
        error: None,
    };
    assert!(res.success);
    assert_eq!(res.status_code, 200);
    assert_eq!(res.duration_ms, 42);
    assert!(res.error.is_none());
}

#[test]
fn webhook_delivery_result_failure_fields() {
    let res = WebhookDeliveryResult {
        success: false,
        status_code: 500,
        response_body: "boom".to_string(),
        response_headers: std::collections::HashMap::new(),
        duration_ms: 0,
        error: Some("timeout".to_string()),
    };
    assert!(!res.success);
    assert_eq!(res.status_code, 500);
    assert_eq!(res.error.as_deref(), Some("timeout"));
}

#[test]
fn webhook_delivery_result_clone() {
    let res = WebhookDeliveryResult {
        success: true,
        status_code: 201,
        response_body: "created".to_string(),
        response_headers: std::collections::HashMap::new(),
        duration_ms: 10,
        error: None,
    };
    let cloned = res.clone();
    assert_eq!(cloned.status_code, res.status_code);
    assert_eq!(cloned.response_body, res.response_body);
}
