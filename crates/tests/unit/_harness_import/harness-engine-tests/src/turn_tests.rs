use std::sync::Arc;
use std::sync::atomic::Ordering;

use harness_engine::{Admitted, Delivery, ErrorKind, NullGovernor};
use harness_store::{FinishReason, PartBody, ToolState};
use serde_json::json;

use crate::support::{
    Scripted, TestGovernor, harness, prompt, text_reply, texts, titled_session, tool_reply,
    tool_state, wait_idle,
};

#[tokio::test]
async fn a_text_turn_streams_into_the_store() {
    let governor = Arc::new(TestGovernor::default());
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("Hello there"))]));
    let harness = harness(
        Arc::<TestGovernor>::clone(&governor),
        Arc::clone(&transport),
    );
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    let admitted = harness
        .engine
        .prompt(prompt(&harness, &id, "Say hello", Delivery::Steer))
        .await
        .unwrap();
    assert!(matches!(admitted, Admitted::Started { .. }));
    wait_idle(&mut events, &id).await;

    let messages = harness.engine.messages(&id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(texts(&messages[0]), "Say hello");
    assert_eq!(texts(&messages[1]), "Hello there");
    assert!(
        messages[1]
            .parts
            .iter()
            .any(|part| matches!(part.body, PartBody::StepStart { .. }))
    );
    assert!(messages[1].parts.iter().any(|part| matches!(
        part.body,
        PartBody::StepFinish {
            reason: FinishReason::Stop,
            ..
        }
    )));
    let assistant = messages[1].message.as_assistant().unwrap();
    assert_eq!(assistant.usage.input, 10);
    assert_eq!(assistant.finish, Some(FinishReason::Stop));
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["model"], "test-model");
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn a_tool_turn_runs_the_tool_and_sends_its_result_back() {
    let governor = Arc::new(TestGovernor::default());
    let transport = Arc::new(Scripted::with(vec![
        (
            200,
            tool_reply("call_1", "read", &json!({"path": "notes.txt"})),
        ),
        (200, text_reply("The notes say hello.")),
    ]));
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
    assert_eq!(messages.len(), 3);
    match tool_state(&messages) {
        ToolState::Completed { output, .. } => {
            assert!(output.contains("hello from the notes file"), "{output}");
        },
        other => panic!("the read call did not complete: {other:?}"),
    }
    assert_eq!(texts(&messages[2]), "The notes say hello.");
    let reviewed = governor.tool_calls.lock().clone();
    assert_eq!(reviewed.len(), 1);
    assert_eq!(reviewed[0].call_id.as_str(), "call_1");
    assert_eq!(reviewed[0].session_id, id);
    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    let sent = requests[1]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        sent.iter()
            .any(|message| message["role"] == "tool" && message["tool_call_id"] == "call_1")
    );
    assert!(
        requests[0]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
    );
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn an_unauthorised_response_is_retried_once_after_the_governor_asks() {
    let governor = Arc::new(TestGovernor {
        retry_status: Some(401),
        ..TestGovernor::default()
    });
    let transport = Arc::new(Scripted::with(vec![
        (401, r#"{"error": {"message": "token expired"}}"#.to_owned()),
        (200, text_reply("After the refresh")),
    ]));
    let harness = harness(
        Arc::<TestGovernor>::clone(&governor),
        Arc::clone(&transport),
    );
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Hello", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;

    let messages = harness.engine.messages(&id).await.unwrap();
    assert_eq!(texts(&messages[1]), "After the refresh");
    assert!(
        messages[1]
            .message
            .as_assistant()
            .is_some_and(|assistant| assistant.error.is_none())
    );
    assert_eq!(governor.authorized.load(Ordering::SeqCst), 2);
    assert_eq!(*governor.responses.lock(), vec![(401, 1), (200, 2)]);
    assert_eq!(transport.requests().len(), 2);
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn without_models_a_prompt_is_refused_before_anything_is_stored() {
    let transport = Arc::new(Scripted::default());
    let harness = harness(Arc::new(NullGovernor), Arc::clone(&transport));
    let id = titled_session(&harness).await;

    let error = harness
        .engine
        .prompt(prompt(&harness, &id, "Hello", Delivery::Steer))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotSignedIn);
    assert_eq!(
        harness.engine.messages(&id).await.unwrap(),
        Vec::<harness_store::MessageWithParts>::new()
    );
    assert_eq!(transport.requests(), Vec::<serde_json::Value>::new());
    assert_eq!(
        harness.engine.models(),
        Vec::<harness_engine::ModelSpec>::new()
    );
    harness.engine.shutdown().await;
}
