use std::sync::Arc;

use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    HttpWebhookBroadcaster, WebhookContext, WebhookError,
};
use systemprompt_identifiers::UserId;
use systemprompt_test_mocks::recording_webhooks;

fn ctx(user: &str, token: impl Into<String>) -> WebhookContext {
    WebhookContext::new(recording_webhooks(), UserId::new(user), token)
}

#[test]
fn webhook_context_stores_user_and_token() {
    let ctx = ctx("user-1", "auth-token-xyz");
    assert_eq!(ctx.user_id(), &UserId::new("user-1"));
    assert_eq!(ctx.auth_token(), "auth-token-xyz");
}

#[test]
fn webhook_context_debug_includes_struct_name() {
    let ctx = ctx("u", "t");
    assert!(format!("{ctx:?}").contains("WebhookContext"));
}

#[test]
fn webhook_context_accepts_string_token() {
    let ctx = ctx("u", String::from("owned"));
    assert_eq!(ctx.user_id().as_str(), "u");
}

#[test]
fn webhook_error_status_display() {
    let err = WebhookError::StatusError {
        status: 500,
        message: "boom".to_string(),
    };
    let s = format!("{err}");
    assert!(s.contains("500"));
    assert!(s.contains("boom"));
}

#[test]
fn http_broadcaster_normalises_a_trailing_slash() {
    let broadcaster = HttpWebhookBroadcaster::new("http://api.internal/").expect("client");
    assert!(format!("{broadcaster:?}").contains("http://api.internal\""));
}

#[tokio::test]
async fn broadcast_returns_error_when_endpoint_unreachable() {
    use systemprompt_models::AgUiEventBuilder;
    let broadcaster = HttpWebhookBroadcaster::new("http://127.0.0.1:9").expect("client");
    let ctx = WebhookContext::new(Arc::new(broadcaster), UserId::new("u1"), "tok");
    let event = AgUiEventBuilder::skill_loaded(
        systemprompt_identifiers::SkillId::new("s1"),
        "name".to_string(),
        Some("desc".to_string()),
        None,
    );
    let result = ctx.broadcast_agui(event).await;
    assert!(matches!(result, Err(WebhookError::Request(_))));
}
