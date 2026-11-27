use std::sync::Arc;

use harness_engine::Delivery;
use serde_json::Value;

use crate::support::{
    Scripted, TestGovernor, harness, harness_with, prompt, text_reply, titled_session, wait_idle,
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
