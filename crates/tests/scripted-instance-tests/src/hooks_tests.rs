use serde_json::json;
use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::hook_events::HookEvent;
use systemprompt_scripted_instance::{
    Event, ModelFixture, RecordedBody, ScriptedInstance, Verdict, VerdictScript,
};

use crate::fixture::{MODEL, Signed};

fn event(name: &str, tool: &str) -> HookEvent {
    serde_json::from_value(json!({
        "hook_event_name": name,
        "session_id": "5b0c1a52-9a44-5d0e-8a0c-3f1d2b7c9e10",
        "native_session_id": "ses_scripted",
        "cwd": "/workspace",
        "native_host": "harness",
        "event_id": "evt-1",
        "tool_name": tool,
        "tool_input": { "path": "README.md" },
        "tool_use_id": "toolu_1",
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tool_verdict_and_a_telemetry_event_reach_the_hook_routes() {
    let verdicts = VerdictScript::allow_all().for_tool(
        "shell",
        Verdict::Deny {
            reason: "no shell here".to_owned(),
        },
    );
    let instance = ScriptedInstance::builder()
        .models([ModelFixture::messages(MODEL)])
        .verdict(verdicts)
        .start()
        .await
        .unwrap();
    let signed = Signed::sign_in(instance).await;
    let plugin = PluginId::new("governance");
    let jwt = signed.jwt().await;
    let hook_client = signed.client.provision_hook_client(&jwt).await.unwrap();
    let grant = signed
        .client
        .mint_hook_token(&hook_client, &plugin)
        .await
        .unwrap();

    let allowed = signed
        .client
        .govern(&grant.token, &plugin, &event("PreToolUse", "read"))
        .await
        .unwrap();
    assert_eq!(allowed.refusal(), None);
    let denied = signed
        .client
        .govern(&grant.token, &plugin, &event("PreToolUse", "shell"))
        .await
        .unwrap();
    assert_eq!(denied.refusal(), Some("no shell here"));
    signed
        .client
        .track(&grant.token, &plugin, &event("SessionStart", "read"), None)
        .await
        .unwrap();

    let verdict = signed.instance.wait_for(Event::Verdict).await.unwrap();
    let RecordedBody::Json(body) = verdict.body else {
        panic!("the verdict request is recorded as JSON");
    };
    assert_eq!(body["tool_name"], "read");
    let telemetry = signed.instance.wait_for(Event::Telemetry).await.unwrap();
    let RecordedBody::Json(body) = telemetry.body else {
        panic!("telemetry is recorded as JSON");
    };
    assert_eq!(body["hook_event_name"], "SessionStart");
    signed.accounts.shutdown();
}
