//! Transient model failures: retried before output with the server's delay,
//! a finite allowance, clean retries of cut streams, and visible failures for
//! errors that are not retried.

use systemprompt_engine::run::{Delivery, ErrorKind};
use systemprompt_engine::store::MessageErrorKind;
use systemprompt_scripted_instance::TurnFault;

use crate::support::{Scripted, Setup, fault, read_turn, settle, text_turn, texts};

#[tokio::test(flavor = "multi_thread")]
async fn a_rate_limit_before_output_is_retried_with_a_visible_retry_status() {
    let scripted =
        Scripted::start(vec![fault(TurnFault::Status(429)), text_turn("Recovered.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(settled.retries, [1]);
    let messages = scripted.messages(&id).await;
    assert_eq!(texts(messages.last().unwrap()), "Recovered.");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upstream_internal_error_is_retried() {
    let scripted = Scripted::start(vec![fault(TurnFault::Status(503)), text_turn("Back.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(scripted.instance.unconsumed_turns(), 0);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_error_is_not_retried_and_ends_the_step_as_a_visible_failure_with_its_status() {
    let scripted = Scripted::start(vec![fault(TurnFault::Status(400)), text_turn("never")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.retries.is_empty());
    assert_eq!(settled.failures.len(), 1);
    assert_eq!(settled.failures[0].kind, ErrorKind::Model);
    assert_eq!(scripted.instance.unconsumed_turns(), 1);
    let messages = scripted.messages(&id).await;
    let reply = messages
        .iter()
        .rev()
        .find_map(|m| m.message.as_assistant())
        .unwrap();
    let error = reply.error.as_ref().expect("the reply records the failure");
    assert_eq!(error.status, Some(400));
    assert!(!error.retryable);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_retry_allowance_is_finite() {
    let turns = (0..8).map(|_| fault(TurnFault::Status(503))).collect();
    let scripted = Scripted::start(turns).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert_eq!(settled.failures.len(), 1);
    let consumed = 8 - scripted.instance.unconsumed_turns();
    assert!((2..8).contains(&consumed), "{consumed} attempts");
    let messages = scripted.messages(&id).await;
    let reply = messages
        .iter()
        .rev()
        .find_map(|m| m.message.as_assistant())
        .unwrap();
    assert_eq!(
        reply.error.as_ref().map(|e| e.kind),
        Some(MessageErrorKind::Api)
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_cut_off_before_any_tool_ran_is_retried_cleanly() {
    let mut cut = text_turn("partial words");
    cut.fault = Some(TurnFault::CloseAfterEvents(2));
    let scripted = Scripted::start(vec![cut, text_turn("Whole answer.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let messages = scripted.messages(&id).await;
    let replies: Vec<String> = messages
        .iter()
        .filter(|m| m.message.as_assistant().is_some())
        .map(texts)
        .collect();
    assert_eq!(replies.last().map(String::as_str), Some("Whole answer."));
    assert!(
        !replies
            .iter()
            .any(|text| text.contains("partial words") && text.contains("Whole"))
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stream_with_an_undecodable_event_is_retried_or_failed_but_never_hangs() {
    let scripted =
        Scripted::start(vec![fault(TurnFault::MalformedEvent), text_turn("Fine.")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let messages = scripted.messages(&id).await;
    let last = messages
        .iter()
        .rev()
        .find_map(|m| m.message.as_assistant())
        .unwrap();
    assert!(last.completed_at.is_some());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retried_model_call_does_not_use_up_an_agent_step() {
    let scripted = Scripted::with(
        vec![
            fault(TurnFault::Status(429)),
            read_turn("t1"),
            text_turn("Done."),
        ],
        Setup {
            settings: Some(r#"{"agent": {"build": {"steps": 2}}}"#.to_owned()),
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
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 3);
    assert!(
        bodies[1]["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_run_does_not_stop_a_later_prompt_from_running() {
    let scripted = Scripted::start(vec![
        fault(TurnFault::Status(400)),
        text_turn("Second try."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "first", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    scripted
        .engine
        .prompt(scripted.prompt(&id, "again", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty());
    assert_eq!(
        texts(scripted.messages(&id).await.last().unwrap()),
        "Second try."
    );
    scripted.finish().await;
}
