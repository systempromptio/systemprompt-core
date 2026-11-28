use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde_json::json;
use systemprompt_engine::run::{Admitted, Delivery, ErrorKind};
use systemprompt_engine::store::{
    AssistantMessage, CallId, FinishReason, JsonMap, MessageError, MessageErrorKind, Part,
    PartBody, ToolState, UserMessage, now_ms,
};

use crate::run::support::{
    NullGovernor, Scripted, TestGovernor, harness, prompt, text_reply, texts, titled_session,
    tool_reply, tool_state, wait_idle,
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
        Vec::<systemprompt_engine::store::MessageWithParts>::new()
    );
    assert_eq!(transport.requests(), Vec::<serde_json::Value>::new());
    assert_eq!(
        harness.engine.models(),
        Vec::<systemprompt_engine::run::ModelSpec>::new()
    );
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn a_failed_reply_is_sent_back_only_up_to_its_last_tool_call_that_ran() {
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("ok"))]));
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;
    let store = harness.engine.store();
    let model = crate::run::support::model().id;
    let question = UserMessage::new(store.ids(), id.clone(), "build", &model);
    let question_id = question.id.clone();
    store.upsert_message(&question.into()).unwrap();
    let text = |body: &str| PartBody::Text {
        text: body.to_owned(),
        synthetic: false,
    };
    store
        .upsert_part(&Part::new(
            store.ids(),
            id.clone(),
            question_id.clone(),
            text("Start"),
        ))
        .unwrap();
    let failed = |parts: Vec<PartBody>| {
        let mut reply = AssistantMessage::new(
            store.ids(),
            id.clone(),
            question_id.clone(),
            "build",
            &model,
        );
        reply.finish = Some(FinishReason::Error);
        reply.completed_at = Some(now_ms());
        reply.error = Some(MessageError {
            kind: MessageErrorKind::Api,
            message: "the stream dropped".to_owned(),
            status: None,
            retryable: false,
        });
        let reply_id = reply.id.clone();
        store.upsert_message(&reply.into()).unwrap();
        for body in parts {
            store
                .upsert_part(&Part::new(store.ids(), id.clone(), reply_id.clone(), body))
                .unwrap();
        }
    };
    failed(vec![text("Partial visible answer")]);
    failed(vec![
        text("Reading first"),
        PartBody::Tool {
            call_id: CallId::try_new("call_ran").unwrap(),
            tool: "read".to_owned(),
            state: Box::new(ToolState::Completed {
                input: json!({"path": "notes.txt"}),
                output: "Hello".to_owned(),
                title: "notes.txt".to_owned(),
                metadata: JsonMap::new(),
                attachments: Vec::new(),
                started: now_ms(),
                ended: now_ms(),
            }),
        },
        text("Now I will run the te"),
    ]);
    let mut events = harness.engine.subscribe();
    harness
        .engine
        .prompt(prompt(&harness, &id, "Try again", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;

    let sent = transport.requests()[0].to_string();
    assert!(
        sent.contains("Reading first") && sent.contains("call_ran"),
        "{sent}"
    );
    assert!(!sent.contains("Partial visible answer"), "{sent}");
    assert!(!sent.contains("run the te"), "{sent}");
    harness.engine.shutdown().await;
}
