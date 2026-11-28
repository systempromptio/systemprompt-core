//! The agent loop against the scripted instance: steps and tool settlement,
//! steering and queueing, interruption, the step limit, and how tool failures
//! and malformed calls reach the model.

use std::time::Duration;

use serde_json::json;
use systemprompt_engine::run::{Admitted, Delivery, EngineEvent, RunStatus};
use systemprompt_engine::store::{MessageErrorKind, ToolState};
use systemprompt_scripted_instance::{Respond, StopReason, ToolCall as Call, Turn};

use crate::support::{
    Scripted, Setup, read_turn, settle, stalled, text_turn, tool_states, tool_turn, until,
    user_texts,
};

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_sends_one_request_per_step_and_settles_each_tool_call_before_continuing() {
    let scripted = Scripted::start(vec![read_turn("t1"), text_turn("Read it.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);

    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    let tools: Vec<&str> = bodies[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(tools.contains(&"read"), "{tools:?}");
    let second = bodies[1].to_string();
    assert!(second.contains("tool_result") && second.contains("hello from the notes file"));
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Completed { .. }]),
        "{states:?}"
    );
    assert_eq!(scripted.instance.unconsumed_turns(), 0);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn several_steers_sent_during_a_step_are_delivered_together_in_one_continuation() {
    let scripted = Scripted::start(vec![
        stalled(read_turn("t1"), 1500),
        text_turn("Both done."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Start", Delivery::Steer))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    for text in ["first steer", "second steer"] {
        let admitted = scripted
            .engine
            .prompt(scripted.prompt(&id, text, Delivery::Steer))
            .await
            .unwrap();
        assert_eq!(admitted, Admitted::Steered);
    }
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2, "one continuation step carries both steers");
    let second = bodies[1].to_string();
    assert!(second.contains("first steer") && second.contains("second steer"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn queued_prompts_are_delivered_one_turn_at_a_time_in_order() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("one"), 1000),
        text_turn("two"),
        text_turn("three"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "p1", Delivery::Queue))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    for text in ["p2", "p3"] {
        let admitted = scripted
            .engine
            .prompt(scripted.prompt(&id, text, Delivery::Queue))
            .await
            .unwrap();
        assert_eq!(admitted, Admitted::Queued);
    }
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 3);
    assert!(!bodies[1].to_string().contains("p3"));
    assert_eq!(
        user_texts(&scripted.messages(&id).await),
        ["p1", "p2", "p3"]
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupting_an_idle_session_does_nothing() {
    let scripted = Scripted::start(vec![]).await;
    let id = scripted.session().await;
    assert!(!scripted.engine.abort(&id));
    assert_eq!(scripted.engine.status(&id), RunStatus::Idle);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupting_a_streaming_step_ends_the_turn_and_marks_the_reply_aborted() {
    let scripted = Scripted::start(vec![stalled(text_turn("partial answer"), 20_000)]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Go", Delivery::Steer))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(scripted.engine.abort(&id));
    let settled = settle(&mut events, &id).await;
    assert!(
        settled.failures.is_empty(),
        "an interrupt is not reported as a failure"
    );
    let messages = scripted.messages(&id).await;
    let reply = messages
        .iter()
        .rev()
        .find_map(|m| m.message.as_assistant())
        .unwrap();
    assert_eq!(
        reply.error.as_ref().map(|error| error.kind),
        Some(MessageErrorKind::Aborted)
    );
    assert!(reply.completed_at.is_some());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an interrupt clears the inbox, so queued prompts are dropped"]
async fn queued_input_survives_an_interrupt_for_a_later_turn() {
    let scripted =
        Scripted::start(vec![stalled(text_turn("slow"), 20_000), text_turn("later")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "first", Delivery::Queue))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "parked", Delivery::Queue))
        .await
        .unwrap();
    scripted.engine.abort(&id);
    settle(&mut events, &id).await;
    assert!(user_texts(&scripted.messages(&id).await).contains(&"parked".to_owned()));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an interrupt before any output leaves the prompt in the session"]
async fn interrupting_before_any_output_takes_the_prompt_back() {
    let silent = Turn {
        respond: Vec::new(),
        stop: Some(StopReason::End),
        fault: Some(systemprompt_scripted_instance::TurnFault::StallMs(20_000)),
        ..Turn::default()
    };
    let scripted = Scripted::start(vec![silent]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "take me back", Delivery::Steer))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    scripted.engine.abort(&id);
    settle(&mut events, &id).await;
    let users = user_texts(&scripted.messages(&id).await);
    assert!(users.is_empty(), "{users:?}");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_allowed_step_is_sent_without_tools_and_ends_the_turn() {
    let settings = r#"{"agent": {"build": {"steps": 2}}}"#;
    let scripted = Scripted::with(
        vec![read_turn("t1"), read_turn("t2"), text_turn("never")],
        Setup {
            settings: Some(settings.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Loop", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[1]["tools"].as_array().is_none_or(Vec::is_empty));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_tool_is_reported_to_the_model_and_the_turn_continues() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "read", json!({"path": "missing.txt"})),
        text_turn("It is missing."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read missing", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Error { .. }]),
        "{states:?}"
    );
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[1].to_string().contains("tool_result"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_configured_denial_is_returned_to_the_model_and_the_turn_continues() {
    let scripted = Scripted::with(
        vec![read_turn("t1"), text_turn("Denied, moving on.")],
        Setup {
            settings: Some(r#"{"permission": {"read": "deny"}}"#.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.asks.is_empty());
    assert!(matches!(
        tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Error { .. }]
    ));
    assert_eq!(scripted.model_bodies().len(), 2);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_to_an_unknown_tool_is_answered_with_an_error_and_the_turn_continues() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "no_such_tool", json!({})),
        text_turn("Sorry."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Try", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[1].to_string().contains("no_such_tool"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn unparseable_tool_input_is_answered_with_an_error_the_model_can_correct() {
    let broken = Turn {
        respond: vec![Respond::ToolCall(Call {
            id: "t1".to_owned(),
            name: "read".to_owned(),
            input: json!({"path": "notes.txt"}),
            argument_chunks: vec!["{\"path\": \"notes".to_owned()],
        })],
        stop: Some(StopReason::ToolUse),
        ..Turn::default()
    };
    let scripted = Scripted::start(vec![broken, read_turn("t2"), text_turn("Fixed.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.first(), Some(ToolState::Error { .. })),
        "{states:?}"
    );
    assert!(
        matches!(states.get(1), Some(ToolState::Completed { .. })),
        "{states:?}"
    );
    assert_eq!(scripted.model_bodies().len(), 3);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn arguments_sent_as_an_encoded_string_are_repaired_before_validation() {
    let encoded = Turn {
        respond: vec![Respond::ToolCall(Call {
            id: "t1".to_owned(),
            name: "read".to_owned(),
            input: json!("{\"path\": \"notes.txt\"}"),
            argument_chunks: Vec::new(),
        })],
        stop: Some(StopReason::ToolUse),
        ..Turn::default()
    };
    let scripted = Scripted::start(vec![encoded, text_turn("Read.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Completed { .. }]),
        "{states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a tool name another agent product uses is not mapped to the matching tool"]
async fn a_tool_name_another_product_uses_runs_as_the_matching_tool() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "Read", json!({"file_path": "notes.txt"})),
        text_turn("Read."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Completed { .. }]),
        "{states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn different_sessions_run_at_the_same_time() {
    let scripted = Scripted::start(vec![stalled(text_turn("a"), 800), text_turn("b")]).await;
    let first = scripted.session().await;
    let second = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&first, "one", Delivery::Steer))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    let admitted = scripted
        .engine
        .prompt(scripted.prompt(&second, "two", Delivery::Steer))
        .await
        .unwrap();
    assert!(matches!(admitted, Admitted::Started { .. }));
    until(&mut events, |event| {
        matches!(event, EngineEvent::Status { session, status: RunStatus::Idle } if *session == second)
    })
    .await;
    assert_eq!(scripted.engine.status(&first), RunStatus::Busy);
    settle(&mut events, &first).await;
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_for_an_unavailable_agent_fails_before_any_model_request() {
    let scripted = Scripted::start(vec![text_turn("unused")]).await;
    let id = scripted.session().await;
    let mut input = scripted.prompt(&id, "hello", Delivery::Steer);
    input.agent = Some("nobody".to_owned());
    assert!(scripted.engine.prompt(input).await.is_err());
    assert!(scripted.model_bodies().is_empty());
    assert!(scripted.messages(&id).await.is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_blank_prompt_is_refused_and_disturbs_nothing() {
    let scripted = Scripted::start(vec![]).await;
    let id = scripted.session().await;
    assert!(
        scripted
            .engine
            .prompt(scripted.prompt(&id, "   ", Delivery::Steer))
            .await
            .is_err()
    );
    assert!(scripted.messages(&id).await.is_empty());
    assert!(scripted.model_bodies().is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_steer_sent_while_a_step_fails_is_still_run() {
    let failing = crate::support::fault(systemprompt_scripted_instance::TurnFault::Status(400));
    let scripted = Scripted::start(vec![
        stalled(read_turn("t0"), 800),
        failing,
        text_turn("Steer seen."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "start", Delivery::Steer))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "late steer", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let users = user_texts(&scripted.messages(&id).await);
    assert!(users.contains(&"late steer".to_owned()), "{users:?}");
    scripted.finish().await;
}
