//! How a step ends when the model stream does not finish cleanly: a refusal
//! by the content filter, a reply cut off at the output limit, and an
//! interruption while a retry waits for its backoff.

use systemprompt_engine::run::{Delivery, EngineEvent, RunStatus};
use systemprompt_engine::store::{FinishReason, Message};
use systemprompt_scripted_instance::{StopReason, Turn, TurnFault};

use crate::support::{Scripted, fault, settle, text_turn, texts, until};

fn stopping(text: &str, stop: StopReason) -> Turn {
    Turn {
        stop: Some(stop),
        ..text_turn(text)
    }
}

fn last_finish(messages: &[systemprompt_engine::store::MessageWithParts]) -> Option<FinishReason> {
    messages
        .iter()
        .rev()
        .find_map(|entry| match &entry.message {
            Message::Assistant(reply) => reply.finish,
            Message::User(_) => None,
        })
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a content-filter finish is recorded as the reply's finish reason and the turn ends without a session failure (engine run/session/step.rs finish_reason)"]
async fn a_content_filter_finish_is_a_visible_failure_that_keeps_the_partial_text() {
    let scripted =
        Scripted::start(vec![stopping("Partial answer", StopReason::ContentFilter)]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert_eq!(
        settled.failures.len(),
        1,
        "the refusal is a visible failure"
    );
    let messages = scripted.messages(&id).await;
    assert_eq!(texts(messages.last().unwrap()), "Partial answer");
    assert_eq!(last_finish(&messages), Some(FinishReason::ContentFilter));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a text reply cut off at the output limit ends the turn with finish Length; the engine sends no continuation request (engine run/session/run.rs turn ends when a step has no tool calls)"]
async fn a_reply_cut_off_at_the_output_limit_is_continued_in_another_request() {
    let scripted = Scripted::start(vec![
        stopping("The first half", StopReason::MaxTokens),
        text_turn(" and the rest."),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Write it all", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        scripted.model_bodies().len(),
        2,
        "the cut reply is continued"
    );
    assert_eq!(scripted.instance.unconsumed_turns(), 0);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_interrupt_during_retry_backoff_starts_no_further_attempt() {
    let scripted =
        Scripted::start(vec![fault(TurnFault::Status(503)), text_turn("never sent")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Hi", Delivery::Steer))
        .await
        .unwrap();
    until(&mut events, |event| {
        matches!(
            event,
            EngineEvent::Status { session, status: RunStatus::Retry { .. } } if session == &id
        )
    })
    .await;
    scripted.engine.abort(&id);
    settle(&mut events, &id).await;
    assert_eq!(
        scripted.instance.unconsumed_turns(),
        1,
        "no attempt follows the interrupt"
    );
    scripted.finish().await;
}
