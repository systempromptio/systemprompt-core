use std::time::{Duration, Instant};

use serde_json::json;
use systemprompt_device::client::ClientError;
use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::hook_events::HookEvent;
use systemprompt_scripted_instance::{
    Event, ModelFixture, RecordedBody, ScriptedInstance, Verdict, VerdictScript,
};

use crate::support::signed_in;

fn event(name: &str, tool: &str) -> HookEvent {
    serde_json::from_value(json!({
        "hook_event_name": name,
        "session_id": "5b0c1a52-9a44-5d0e-8a0c-3f1d2b7c9e10",
        "native_session_id": "ses_scripted",
        "cwd": "/workspace",
        "native_host": "harness",
        "event_id": "evt-1",
        "tool_name": tool,
        "tool_input": { "path": "src/lib.rs" },
        "tool_use_id": "toolu_1",
    }))
    .unwrap()
}

#[tokio::test]
async fn hook_calls_are_recorded_and_verdicts_follow_the_script() {
    let verdicts = VerdictScript::allow_all()
        .for_tool(
            "bash",
            Verdict::Deny {
                reason: "no shell here".to_owned(),
            },
        )
        .for_tool("edit", Verdict::Error { status: 503 })
        .for_tool("fetch", Verdict::Delay { millis: 150 });
    let instance = ScriptedInstance::builder()
        .models([ModelFixture::messages("m-a")])
        .verdict(verdicts)
        .start()
        .await
        .unwrap();
    let (client, session) = signed_in(&instance).await;
    let plugin = PluginId::new("governance");

    let hook_client = client.provision_hook_client(&session.jwt).await.unwrap();
    let grant = client.mint_hook_token(&hook_client, &plugin).await.unwrap();
    assert!(grant.expires_in > Duration::from_secs(300));

    client
        .track(&grant.token, &plugin, &event("SessionStart", "read"), None)
        .await
        .unwrap();
    let telemetry = instance.wait_for(Event::Telemetry).await.unwrap();
    assert_eq!(telemetry.query.as_deref(), Some("plugin_id=governance"));
    let RecordedBody::Json(body) = telemetry.body else {
        panic!("telemetry is recorded as JSON");
    };
    assert_eq!(body["hook_event_name"], "SessionStart");

    let allowed = client
        .govern(&grant.token, &plugin, &event("PreToolUse", "read"))
        .await
        .unwrap();
    assert_eq!(allowed.refusal(), None);
    let denied = client
        .govern(&grant.token, &plugin, &event("PreToolUse", "bash"))
        .await
        .unwrap();
    assert_eq!(denied.refusal(), Some("no shell here"));
    let failed: ClientError = client
        .govern(&grant.token, &plugin, &event("PreToolUse", "edit"))
        .await
        .unwrap_err();
    assert_eq!(failed.status(), Some(503));
    let began = Instant::now();
    let delayed = client
        .govern(&grant.token, &plugin, &event("PreToolUse", "fetch"))
        .await
        .unwrap();
    assert_eq!(delayed.refusal(), None);
    assert!(began.elapsed() >= Duration::from_millis(150));

    let verdict = instance.wait_for(Event::Verdict).await.unwrap();
    assert!(verdict.sequence > telemetry.sequence);
}

#[tokio::test]
async fn a_session_token_is_not_a_hook_token() {
    let instance = ScriptedInstance::builder().start().await.unwrap();
    let (_, session) = signed_in(&instance).await;
    let answer = reqwest::Client::new()
        .post(format!(
            "{}{}",
            instance.base_url(),
            systemprompt_models::ApiPaths::PUBLIC_HOOKS_GOVERN
        ))
        .bearer_auth(session.jwt.expose())
        .json(&event("PreToolUse", "read"))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status().as_u16(), 401);
}
