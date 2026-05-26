use systemprompt_agent::services::a2a_server::streaming::webhook_client::{
    WebhookContext, WebhookError,
};
use systemprompt_identifiers::UserId;

#[test]
fn webhook_context_stores_user_and_token() {
    let user_id = UserId::new("user-1");
    let ctx = WebhookContext::new(user_id.clone(), "auth-token-xyz");
    assert_eq!(ctx.user_id(), &user_id);
}

#[test]
fn webhook_context_clone_preserves_fields() {
    let user_id = UserId::new("user-clone");
    let ctx = WebhookContext::new(user_id.clone(), "tok");
    let cloned = ctx.clone();
    assert_eq!(cloned.user_id(), &user_id);
}

#[test]
fn webhook_context_debug_includes_struct_name() {
    let ctx = WebhookContext::new(UserId::new("u"), "t");
    assert!(format!("{:?}", ctx).contains("WebhookContext"));
}

#[test]
fn webhook_context_accepts_string_token() {
    let ctx = WebhookContext::new(UserId::new("u"), String::from("owned"));
    assert_eq!(ctx.user_id().as_str(), "u");
}

#[test]
fn webhook_error_status_display() {
    let err = WebhookError::StatusError {
        status: 500,
        message: "boom".to_string(),
    };
    let s = format!("{}", err);
    assert!(s.contains("500"));
    assert!(s.contains("boom"));
}

#[test]
fn webhook_error_status_debug() {
    let err = WebhookError::StatusError {
        status: 401,
        message: "unauthorized".to_string(),
    };
    let s = format!("{:?}", err);
    assert!(s.contains("StatusError"));
}

#[tokio::test]
async fn broadcast_returns_error_when_endpoint_unreachable() {
    use systemprompt_models::AgUiEventBuilder;
    let user_id = UserId::new("u1");
    let ctx = WebhookContext::new(user_id, "tok");
    let event = AgUiEventBuilder::skill_loaded(
        systemprompt_identifiers::SkillId::new("s1"),
        "name".to_string(),
        Some("desc".to_string()),
        None,
    );
    // Connection will fail because no api server is running on the
    // config-derived URL — exercises the request error branch.
    let result = ctx.broadcast_agui(event).await;
    assert!(result.is_err());
}
