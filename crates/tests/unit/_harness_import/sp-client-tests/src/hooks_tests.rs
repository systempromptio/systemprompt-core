use std::time::Duration;

use chrono::{TimeDelta, Utc};
use serde_json::json;
use sp_client::{ClientError, HookClient, HookGrant};
use sp_protocol::ids::{NativeSessionId, PluginId};
use sp_protocol::wire::hooks::{HookContext, HookEvent};
use sp_protocol::wire::paths;
use sp_protocol::{DeviceCredential, HookToken};
use url::Url;

use crate::support::{FakeInstance, FakeRequest, FakeResponse, jwt};

fn context() -> HookContext {
    HookContext {
        session_id: NativeSessionId::new("6f1b4c1e-0000-5000-8000-000000000000"),
        native_session_id: "ses_1".to_owned(),
        cwd: "C:\\work".to_owned(),
        native_host: sp_protocol::HOST_ID.to_owned(),
    }
}

fn answer(request: &FakeRequest) -> FakeResponse {
    match request.path() {
        paths::OAUTH_CLIENT => FakeResponse::json(
            200,
            &json!({
                "client_id": "client_abc",
                "client_secret": "s3cret",
                "scopes": ["hook:track"],
                "token_endpoint": "https://evil.example/oauth/token"
            }),
        ),
        "/oauth/token" => FakeResponse::json(
            200,
            &json!({ "access_token": "hook-tok", "token_type": "Bearer", "expires_in": 3600 }),
        ),
        paths::HOOKS_TRACK => FakeResponse::status(202),
        paths::HOOKS_GOVERN => FakeResponse::json(
            200,
            &json!({ "hookSpecificOutput": { "permissionDecision": "deny", "permissionDecisionReason": "No shell here" } }),
        ),
        _ => FakeResponse::status(404),
    }
}

#[tokio::test]
async fn hook_clients_mint_tokens_and_report_events() {
    let fake = FakeInstance::start(answer).await;
    let client = fake.client();

    let hook_client = client.provision_hook_client(&jwt()).await.unwrap();
    assert_eq!(
        hook_client.token_endpoint.as_str(),
        format!("{}/oauth/token", fake.url)
    );
    assert!(!format!("{hook_client:?}").contains("s3cret"));

    let plugin = PluginId::new("governance");
    let grant = client.mint_hook_token(&hook_client, &plugin).await.unwrap();
    assert_eq!(grant.token.expose(), "hook-tok");
    assert_eq!(grant.expires_in, Duration::from_secs(3600));
    assert!(!grant.needs_refresh(Utc::now()));
    let minted = fake.last();
    let form = minted.text();
    assert!(form.contains("grant_type=client_credentials"));
    assert!(form.contains("plugin_id=governance"));
    assert!(form.contains("audience=hook"));

    let foreign = HookClient {
        token_endpoint: Url::parse("https://evil.example/token").unwrap(),
        ..hook_client
    };
    assert!(matches!(
        client.mint_hook_token(&foreign, &plugin).await,
        Err(ClientError::Origin(_))
    ));

    let token = HookToken::new("hook-tok").unwrap();
    let device = DeviceCredential::new("sp_device_abc").unwrap();
    let start = HookEvent::SessionStart {
        context: context(),
        event_id: "evt-1".to_owned(),
    };
    client
        .track(&token, &plugin, &start, Some(&device))
        .await
        .unwrap();
    let tracked = fake.last();
    assert_eq!(tracked.query(), "plugin_id=governance");
    assert_eq!(tracked.header("authorization"), Some("Bearer hook-tok"));
    assert_eq!(tracked.header("x-systemprompt-host"), Some("harness"));
    assert_eq!(tracked.header("x-ingestion-event-id"), Some("evt-1"));
    assert_eq!(
        tracked.header("x-systemprompt-device-credential"),
        Some("sp_device_abc")
    );
    assert_eq!(tracked.json()["hook_event_name"], "SessionStart");

    let before = HookEvent::PreToolUse {
        context: context(),
        tool_name: "bash".to_owned(),
        tool_input: json!({ "command": "ls" }),
        tool_use_id: "call_1".to_owned(),
    };
    let answer = client.govern(&token, &plugin, &before).await.unwrap();
    assert_eq!(answer.refusal(), Some("No shell here"));
    assert!(
        fake.last()
            .header("x-systemprompt-device-credential")
            .is_none()
    );
}

#[test]
fn grants_ask_for_a_refresh_before_they_lapse() {
    let now = Utc::now();
    let grant = |lifetime: i64| HookGrant {
        token: HookToken::new("hook-tok").unwrap(),
        expires_in: Duration::from_secs(lifetime.unsigned_abs()),
        expires_at: now + TimeDelta::try_seconds(lifetime).unwrap(),
    };
    assert!(!grant(3600).needs_refresh(now));
    assert!(grant(1).needs_refresh(now));
    assert!(grant(-60).needs_refresh(now));
}
