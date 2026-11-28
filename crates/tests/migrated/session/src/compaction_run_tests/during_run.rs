//! A compaction requested while a turn is going or beside other prompts: where
//! it runs relative to steers and queued prompts, and what a failed or
//! interrupted compaction does to the work waiting behind it.

use serde_json::json;
use systemprompt_engine::run::{
    Admitted, AttachmentInput, CommandInput, CommandOutcome, Delivery, EngineEvent, ErrorKind,
    Purpose, QuestionId, RunStatus,
};
use systemprompt_engine::store::SessionId;
use systemprompt_scripted_instance::{Event, Turn, TurnFault};
use tokio::sync::broadcast;

use super::{ask, boundaries, compact, compaction_started, last_reply, seed_exchange, summaries};
use crate::support::{
    Scripted, fault, purposes, settle, stalled, text_turn, texts, tool_turn, until, user_texts,
};

fn question_turn() -> Turn {
    tool_turn(
        "q1",
        "question",
        json!({"questions": [{"question": "Which store?", "header": "Store", "options": [
            {"label": "Memory", "description": "nothing kept"},
            {"label": "Disk", "description": "kept between runs"}
        ]}]}),
    )
}

/// Starts a turn whose first step is still streaming when this returns.
async fn active_text_step(scripted: &Scripted, session: &SessionId) {
    scripted
        .engine
        .prompt(scripted.prompt(session, "Active work", Delivery::Steer))
        .await
        .unwrap();
    scripted.instance.wait_for(Event::ModelCall).await.unwrap();
}

/// Starts a turn and returns once its first step is waiting on a question, so
/// a tool call is in flight until the question is answered.
async fn active_tool_step(
    scripted: &Scripted,
    session: &SessionId,
    events: &mut broadcast::Receiver<EngineEvent>,
) -> QuestionId {
    scripted
        .engine
        .prompt(scripted.prompt(session, "Active work", Delivery::Steer))
        .await
        .unwrap();
    let asked = until(events, |event| {
        matches!(event, EngineEvent::QuestionAsked(_))
    })
    .await;
    let EngineEvent::QuestionAsked(question) = asked else {
        unreachable!()
    };
    question.id
}

fn answer(scripted: &Scripted, question: &QuestionId) {
    let answers = Some(vec![vec!["Disk".to_owned()]]);
    assert!(scripted.engine.answer_question(question, answers));
}

async fn steer(scripted: &Scripted, session: &SessionId, text: &str) {
    let admitted = scripted
        .engine
        .prompt(scripted.prompt(session, text, Delivery::Steer))
        .await
        .unwrap();
    assert_eq!(admitted, Admitted::Steered);
}

async fn queue(scripted: &Scripted, session: &SessionId, text: &str) {
    let admitted = scripted
        .engine
        .prompt(scripted.prompt(session, text, Delivery::Queue))
        .await
        .unwrap();
    assert_eq!(admitted, Admitted::Queued);
}

async fn queue_compaction(scripted: &Scripted, session: &SessionId) {
    let command = CommandInput {
        session_id: session.clone(),
        directory: scripted.project.clone(),
        name: "compact".to_owned(),
        arguments: String::new(),
        agent: None,
        model: None,
        delivery: Delivery::Queue,
    };
    let outcome = scripted.engine.command(command).await.unwrap();
    assert_eq!(outcome, CommandOutcome::Compacting);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a compaction requested while a turn is going is refused as busy, not held for a step boundary"]
async fn a_compaction_requested_beside_two_steers_runs_first_at_the_next_boundary() {
    for position in 0..3 {
        let scripted = Scripted::start(vec![
            stalled(text_turn("Active complete"), 1500),
            text_turn("SUMMARY of the active work"),
            text_turn("Steers complete"),
        ])
        .await;
        let id = scripted.session().await;
        let mut events = scripted.engine.subscribe();
        active_text_step(&scripted, &id).await;
        for (index, text) in ["STEER_A", "STEER_B"].into_iter().enumerate() {
            if index == position {
                scripted.engine.compact(&id).await.unwrap();
            }
            steer(&scripted, &id, text).await;
        }
        if position == 2 {
            scripted.engine.compact(&id).await.unwrap();
        }
        settle(&mut events, &id).await;

        assert_eq!(
            purposes(&scripted),
            [Purpose::Step, Purpose::Compaction, Purpose::Step],
            "requested at position {position}"
        );
        let bodies = scripted.model_bodies();
        let summary = bodies[1].to_string();
        assert!(!summary.contains("STEER_A") && !summary.contains("STEER_B"));
        let continuation = bodies[2].to_string();
        assert!(continuation.contains("STEER_A") && continuation.contains("STEER_B"));
        scripted.finish().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a compaction requested while a turn is going is refused as busy, not held for a step boundary"]
async fn a_compaction_requested_during_a_tool_call_waits_for_it_and_precedes_the_steers() {
    let scripted = Scripted::start(vec![
        question_turn(),
        text_turn("SUMMARY of the tool work"),
        text_turn("Steers complete"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    let question = active_tool_step(&scripted, &id, &mut events).await;
    steer(&scripted, &id, "STEER_A").await;
    steer(&scripted, &id, "STEER_B").await;
    scripted.engine.compact(&id).await.unwrap();
    assert_eq!(
        scripted.model_bodies().len(),
        1,
        "nothing is sent while the tool call is in flight"
    );
    answer(&scripted, &question);
    settle(&mut events, &id).await;

    assert_eq!(
        purposes(&scripted),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let bodies = scripted.model_bodies();
    assert!(bodies[1].to_string().contains("tool_result"));
    let continuation = bodies[2].to_string();
    assert!(continuation.contains("STEER_A") && continuation.contains("STEER_B"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a compaction requested while a turn is going is refused as busy, not held for a step boundary"]
async fn steers_sent_before_a_failing_compaction_are_still_delivered() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("Active complete"), 1500),
        fault(TurnFault::Status(400)),
        text_turn("Steers complete"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    active_text_step(&scripted, &id).await;
    steer(&scripted, &id, "STEER_A").await;
    steer(&scripted, &id, "STEER_B").await;
    scripted.engine.compact(&id).await.unwrap();
    let settled = settle(&mut events, &id).await;

    assert_eq!(settled.failures.len(), 1, "the failed summary is reported");
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 3);
    let continuation = bodies[2].to_string();
    assert!(continuation.contains("STEER_A") && continuation.contains("STEER_B"));
    assert_eq!(
        user_texts(&scripted.messages(&id).await),
        ["Active work", "STEER_A", "STEER_B"]
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an interrupt clears the inbox, so steers held behind a compaction are dropped"]
async fn steers_held_behind_an_interrupted_compaction_reach_the_next_turn() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("SUMMARY never finished"), 1500),
        text_turn("Recovered steers"),
    ])
    .await;
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier work",
        "Earlier answer",
        0,
    );
    let mut changes = scripted.engine.store().subscribe();
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    compaction_started(&mut changes).await;
    steer(&scripted, &id, "STEER_A").await;
    steer(&scripted, &id, "STEER_B").await;
    assert!(scripted.engine.abort(&id));
    settle(&mut events, &id).await;
    assert!(summaries(&scripted.messages(&id).await).is_empty());

    ask(
        &scripted.engine,
        scripted.prompt(&id, "carry on", Delivery::Steer),
    )
    .await;
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("STEER_A") && last.contains("STEER_B"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a compaction requested while a turn is going is refused as busy, not held for a step boundary"]
async fn a_queued_compaction_runs_after_the_steers_and_before_later_queued_prompts() {
    let scripted = Scripted::start(vec![
        question_turn(),
        text_turn("Steer complete"),
        text_turn("SUMMARY kept"),
        text_turn("Queue complete"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    let question = active_tool_step(&scripted, &id, &mut events).await;
    queue_compaction(&scripted, &id).await;
    steer(&scripted, &id, "Steer after compaction").await;
    queue(&scripted, &id, "Queue after compaction").await;
    answer(&scripted, &question);
    settle(&mut events, &id).await;

    assert_eq!(
        purposes(&scripted),
        [
            Purpose::Step,
            Purpose::Step,
            Purpose::Compaction,
            Purpose::Step
        ]
    );
    let bodies = scripted.model_bodies();
    assert!(bodies[1].to_string().contains("Steer after compaction"));
    let last = bodies[3].to_string();
    assert!(last.contains("Queue after compaction") && last.contains("SUMMARY kept"));
    assert_eq!(summaries(&scripted.messages(&id).await), ["SUMMARY kept"]);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a compaction requested while a turn is going is refused as busy, not held for a step boundary"]
async fn a_compaction_requested_during_a_tool_turn_runs_before_its_continuation() {
    let scripted = Scripted::start(vec![
        question_turn(),
        text_turn("SUMMARY kept"),
        text_turn("Continued"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    let question = active_tool_step(&scripted, &id, &mut events).await;
    scripted.engine.compact(&id).await.unwrap();
    answer(&scripted, &question);
    settle(&mut events, &id).await;

    assert_eq!(
        purposes(&scripted),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let messages = scripted.messages(&id).await;
    assert_eq!(summaries(&messages), ["SUMMARY kept"]);
    assert_eq!(texts(last_reply(&messages)), "Continued");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_compaction_queued_on_an_idle_session_runs_at_once() {
    let scripted = Scripted::start(vec![text_turn("SUMMARY of the idle session")]).await;
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    let mut events = scripted.engine.subscribe();
    queue_compaction(&scripted, &id).await;
    let settled = settle(&mut events, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(purposes(&scripted), [Purpose::Compaction]);
    let messages = scripted.messages(&id).await;
    assert_eq!(boundaries(&messages), [false]);
    assert_eq!(summaries(&messages), ["SUMMARY of the idle session"]);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a compaction requested while a turn is going is refused as busy, not held for a step boundary"]
async fn a_compaction_queued_behind_a_queued_prompt_runs_after_it() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("one"), 1500),
        text_turn("two"),
        text_turn("SUMMARY of both"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    active_text_step(&scripted, &id).await;
    queue(&scripted, &id, "Queue for later").await;
    queue_compaction(&scripted, &id).await;
    settle(&mut events, &id).await;

    assert_eq!(
        purposes(&scripted),
        [Purpose::Step, Purpose::Step, Purpose::Compaction]
    );
    let bodies = scripted.model_bodies();
    assert!(bodies[1].to_string().contains("Queue for later"));
    assert!(bodies[2].to_string().contains("Queue for later"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_sent_right_after_a_compaction_request_waits_for_the_summary() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("SUMMARY before the follow-up"), 1000),
        text_turn("Follow-up answer"),
    ])
    .await;
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    assert_eq!(scripted.engine.status(&id), RunStatus::Busy);
    steer(&scripted, &id, "Follow up").await;
    let settled = settle(&mut events, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        purposes(&scripted),
        [Purpose::Compaction, Purpose::Step],
        "the prompt is not sent until the summary has come back"
    );
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("SUMMARY before the follow-up") && last.contains("Follow up"));
    assert!(!last.contains("Earlier question"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_sent_behind_a_refused_compaction_still_runs() {
    let scripted =
        Scripted::start(vec![fault(TurnFault::Status(400)), text_turn("Released")]).await;
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Follow up", Delivery::Steer))
        .await
        .unwrap();
    let mut failures = settle(&mut events, &id).await.failures;
    if scripted.instance.unconsumed_turns() > 0 {
        failures.extend(settle(&mut events, &id).await.failures);
    }

    assert_eq!(
        failures.len(),
        1,
        "only the compaction failed: {failures:?}"
    );
    let messages = scripted.messages(&id).await;
    assert_eq!(texts(last_reply(&messages)), "Released");
    assert!(summaries(&messages).is_empty());
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("Earlier question") && last.contains("Follow up"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_that_fails_before_admission_does_not_hold_back_later_work() {
    let scripted = Scripted::start(vec![
        text_turn("SUMMARY after the failed prompt"),
        text_turn("Follow-up answer"),
    ])
    .await;
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    let mut failing = scripted.prompt(&id, "First", Delivery::Steer);
    failing.attachments = vec![AttachmentInput::Path("missing.png".into())];
    let refused = scripted.engine.prompt(failing).await.unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::InvalidInput);
    assert_eq!(scripted.engine.status(&id), RunStatus::Idle);
    assert!(scripted.model_bodies().is_empty());

    let compacted = compact(&scripted.engine, &id).await;
    assert!(compacted.failures.is_empty(), "{:?}", compacted.failures);
    let followed = ask(
        &scripted.engine,
        scripted.prompt(&id, "Follow up", Delivery::Steer),
    )
    .await;
    assert!(followed.failures.is_empty(), "{:?}", followed.failures);
    assert_eq!(purposes(&scripted), [Purpose::Compaction, Purpose::Step]);
    let users = user_texts(&scripted.messages(&id).await);
    assert!(!users.contains(&"First".to_owned()), "{users:?}");
    assert!(users.contains(&"Follow up".to_owned()), "{users:?}");
    scripted.finish().await;
}
