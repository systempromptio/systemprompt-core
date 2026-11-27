use std::sync::Arc;

use harness_engine::{Delivery, Reply, denied};
use harness_prompt::notices::POLICY_DENIAL_PREFIX;
use harness_store::ToolState;
use serde_json::{Value, json};

use crate::support::{
    Scripted, TestGovernor, harness, harness_with, prompt, text_reply, titled_session, tool_reply,
    tool_state, wait_idle, wait_idle_answering,
};

fn read_then(reply: &str) -> Arc<Scripted> {
    Arc::new(Scripted::with(vec![
        (
            200,
            tool_reply("call_1", "read", &json!({"path": "notes.txt"})),
        ),
        (200, text_reply(reply)),
    ]))
}

fn marker<'a>(metadata: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    metadata.get(key).and_then(Value::as_str)
}

#[tokio::test]
async fn a_policy_denial_reaches_the_model_and_the_tool_does_not_run() {
    let reason = "reading files is blocked here";
    let governor = Arc::new(TestGovernor {
        deny: Some(reason.to_owned()),
        ..TestGovernor::default()
    });
    let transport = read_then("Understood.");
    let harness = harness(governor, Arc::clone(&transport));
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
        ToolState::Error {
            error, metadata, ..
        } => {
            assert_eq!(marker(&metadata, denied::KEY), Some(denied::POLICY));
            assert_eq!(marker(&metadata, denied::REASON), Some(reason));
            assert!(error.starts_with(POLICY_DENIAL_PREFIX), "{error}");
            assert!(error.contains(reason));
            assert!(!error.contains("hello from the notes file"));
        },
        other => panic!("the read call was not denied: {other:?}"),
    }
    assert_eq!(transport.requests().len(), 2);
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn a_rule_denial_is_marked_with_the_rule_text() {
    let governor = Arc::new(TestGovernor::default());
    let transport = read_then("Understood.");
    let settings = r#"{"permission": {"read": {"*": "allow", "*notes.txt": "deny"}}}"#;
    let harness = harness_with(
        Arc::<TestGovernor>::clone(&governor),
        Arc::clone(&transport),
        Some(settings),
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
        ToolState::Error {
            error, metadata, ..
        } => {
            assert_eq!(marker(&metadata, denied::KEY), Some(denied::RULE));
            let reason = marker(&metadata, denied::REASON).unwrap_or_default();
            assert!(reason.starts_with("a rule denies read for"), "{reason}");
            assert!(error.ends_with(reason), "{error}");
        },
        other => panic!("the read call was not denied by the rule: {other:?}"),
    }
    assert!(governor.tool_calls.lock().is_empty());
    assert_eq!(transport.requests().len(), 2);
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn a_rejected_permission_stops_the_turn() {
    let governor = Arc::new(TestGovernor::default());
    let transport = read_then("should not be sent");
    let settings = r#"{"permission": {"read": "ask"}}"#;
    let harness = harness_with(governor, Arc::clone(&transport), Some(settings));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    let asked = wait_idle_answering(&mut events, &id, |ask| {
        assert_eq!(ask.permission, "read");
        assert_eq!(ask.call_id.as_str(), "call_1");
        assert_eq!(harness.engine.pending_permissions().len(), 1);
        assert!(
            harness
                .engine
                .reply_permission(&ask.id, Reply::Reject { feedback: None })
        );
    })
    .await;
    assert_eq!(asked, 1);

    let messages = harness.engine.messages(&id).await.unwrap();
    match tool_state(&messages) {
        ToolState::Error {
            error, metadata, ..
        } => {
            assert!(error.contains("declined"), "{error}");
            assert_eq!(marker(&metadata, denied::KEY), Some(denied::REJECTED));
            assert!(metadata.get(denied::FEEDBACK).is_none());
        },
        other => panic!("the read call was not rejected: {other:?}"),
    }
    assert_eq!(transport.requests().len(), 1);
    assert_eq!(
        harness.engine.pending_permissions(),
        Vec::<harness_engine::PermissionAsk>::new()
    );
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn an_aborted_call_is_marked_aborted() {
    let governor = Arc::new(TestGovernor::default());
    let transport = read_then("should not be sent");
    let settings = r#"{"permission": {"read": "ask"}}"#;
    let harness = harness_with(governor, Arc::clone(&transport), Some(settings));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    let asked = wait_idle_answering(&mut events, &id, |_| {
        assert!(harness.engine.abort(&id));
    })
    .await;
    assert_eq!(asked, 1);

    let messages = harness.engine.messages(&id).await.unwrap();
    match tool_state(&messages) {
        ToolState::Error { metadata, .. } => {
            assert_eq!(marker(&metadata, denied::KEY), Some(denied::ABORTED));
            assert!(metadata.get(denied::REASON).is_none());
        },
        other => panic!("the read call was not aborted: {other:?}"),
    }
    assert_eq!(transport.requests().len(), 1);
    assert_eq!(
        harness.engine.pending_permissions(),
        Vec::<harness_engine::PermissionAsk>::new()
    );
    harness.engine.shutdown().await;
}
