use std::sync::Arc;

use serde_json::{Value, json};
use systemprompt_engine::run::Delivery;
use systemprompt_engine::store::{FinishReason, Message, ToolState};

use crate::run::support::{
    Scripted, TestGovernor, harness, harness_with, prompt, text_reply, titled_session, tool_reply,
    tool_state, wait_idle,
};

async fn sent_max_tokens(settings: Option<&str>) -> Value {
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("ok"))]));
    let governor = Arc::new(TestGovernor::default());
    let harness = match settings {
        Some(settings) => harness_with(governor, Arc::clone(&transport), Some(settings)),
        None => harness(governor, Arc::clone(&transport)),
    };
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();
    harness
        .engine
        .prompt(prompt(&harness, &id, "Say ok", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;
    harness.engine.shutdown().await;
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests[0]["max_tokens"].clone()
}

#[tokio::test]
async fn the_agents_output_limit_caps_each_step() {
    let sent = sent_max_tokens(Some(r#"{"agent": {"build": {"maxOutput": 16}}}"#)).await;
    assert_eq!(sent, 16);
}

#[tokio::test]
async fn an_output_limit_above_the_models_ceiling_is_held_to_the_ceiling() {
    let sent = sent_max_tokens(Some(r#"{"agent": {"build": {"maxOutput": 100000}}}"#)).await;
    assert_eq!(sent, 4_096);
}

#[tokio::test]
async fn without_an_output_limit_the_models_ceiling_applies() {
    let sent = sent_max_tokens(None).await;
    assert_eq!(sent, 4_096);
}

#[tokio::test]
async fn a_step_cut_off_at_the_output_limit_settles_its_calls_without_running_them() {
    let cut = tool_reply("call_1", "read", &json!({"path": "notes.txt"})).replace(
        "\"finish_reason\":\"tool_calls\"",
        "\"finish_reason\":\"length\"",
    );
    let transport = Arc::new(Scripted::with(vec![
        (200, cut),
        (200, text_reply("unused")),
    ]));
    let governor = Arc::new(TestGovernor::default());
    let harness = harness(
        Arc::<TestGovernor>::clone(&governor),
        Arc::clone(&transport),
    );
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();
    harness
        .engine
        .prompt(prompt(&harness, &id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;

    let messages = harness.engine.messages(&id).await.unwrap();
    match tool_state(&messages) {
        ToolState::Error { error, .. } => assert!(error.contains("output limit"), "{error}"),
        other => panic!("the cut-off call was not settled as failed: {other:?}"),
    }
    let finish = messages
        .iter()
        .rev()
        .find_map(|entry| match &entry.message {
            Message::Assistant(reply) => reply.finish,
            Message::User(_) => None,
        });
    assert_eq!(finish, Some(FinishReason::Length));
    assert_eq!(transport.requests().len(), 1, "the turn ends at the limit");
    assert!(
        governor.tool_calls.lock().is_empty(),
        "the call was never reviewed"
    );
    harness.engine.shutdown().await;
}
