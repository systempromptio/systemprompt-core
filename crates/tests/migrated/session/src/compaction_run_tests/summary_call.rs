//! The summary call of a manual compaction: what it is sent, which replies
//! count as a summary, how transient failures are retried, and how a refused,
//! interrupted or unpreparable compaction leaves the session.

use serde_json::{Value, json};
use systemprompt_engine::prompt::compaction::SECTIONS;
use systemprompt_engine::run::{Delivery, ErrorKind, RunStatus};
use systemprompt_engine::store::{Message, MessageId, Store};
use systemprompt_scripted_instance::{StopReason, Turn, TurnFault};

use super::{
    ask, boundaries, compact, compaction_started, seed_exchange, seed_reply, seed_user, summaries,
};
use crate::support::{
    MODEL, Scripted, Setup, fault, read_turn, settle, stalled, text_turn, texts, tool_states,
    tool_turn, user_texts, with_usage,
};

/// A session holding one finished exchange that no model call produced.
async fn with_history(scripted: &Scripted) -> systemprompt_engine::store::SessionId {
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    id
}

/// A reply with every section the summary request asks for.
fn sectioned_summary() -> String {
    let mut summary = String::new();
    for section in SECTIONS {
        summary.push_str("## ");
        summary.push_str(section);
        summary.push_str("\nNone\n\n");
    }
    summary.trim_end().to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_compaction_works_when_the_model_states_no_context_window() {
    let scripted = Scripted::with(
        vec![
            text_turn("Earlier answer"),
            text_turn("SUMMARY without a window"),
        ],
        Setup {
            context_window: Some(0),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    ask(
        &scripted.engine,
        scripted.prompt(&id, "Earlier question", Delivery::Steer),
    )
    .await;
    let settled = compact(&scripted.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[1].to_string().contains("Earlier question"));
    let messages = scripted.messages(&id).await;
    assert_eq!(boundaries(&messages), [false]);
    assert_eq!(summaries(&messages), ["SUMMARY without a window"]);
    scripted.finish().await;
}

/// The last step request before a compaction and the summary request, for a
/// manual compaction or one the context ceiling started.
async fn step_and_summary_requests(auto: bool) -> (Value, Value) {
    let mut turns = vec![
        read_turn("t1"),
        with_usage(text_turn("Earlier answer"), 9_500, 100),
        text_turn("SUMMARY of the review"),
    ];
    if auto {
        turns.push(text_turn("Continued"));
    }
    let scripted = Scripted::with(
        turns,
        Setup {
            context_window: Some(10_000),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    ask(
        &scripted.engine,
        scripted.prompt(&id, "Review these changes", Delivery::Steer),
    )
    .await;
    if auto {
        ask(
            &scripted.engine,
            scripted.prompt(&id, "Retained recent request", Delivery::Steer),
        )
        .await;
    } else {
        compact(&scripted.engine, &id).await;
    }
    let bodies = scripted.model_bodies();
    scripted.finish().await;
    (bodies[1].clone(), bodies[2].clone())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the summary request carries the compaction agent's own system prompt and no tools, not the session's request prefix"]
async fn the_summary_request_repeats_the_sessions_request_prefix() {
    for auto in [false, true] {
        let (step, summary) = step_and_summary_requests(auto).await;
        for field in ["model", "system", "tools", "max_tokens"] {
            assert_eq!(summary[field], step[field], "{field} (auto: {auto})");
        }
        let before = step["messages"].as_array().unwrap();
        let after = summary["messages"].as_array().unwrap();
        assert!(after.len() > before.len());
        assert_eq!(&after[..before.len()], before.as_slice());
        assert_eq!(after.last().unwrap()["role"], "user");
        assert_eq!(
            summary.to_string().contains("Retained recent request"),
            auto
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: any text is accepted as the summary and a reply without text ends the compaction silently; no reminder is sent"]
async fn a_reply_that_is_not_a_summary_gets_one_reminder_before_the_compaction_fails() {
    let not_summaries = [
        tool_turn("c1", "read", json!({"path": "notes.txt"})),
        text_turn("Let me look around the project first."),
    ];
    for reply in not_summaries {
        for recovers in [true, false] {
            let second = if recovers {
                text_turn(&sectioned_summary())
            } else {
                reply.clone()
            };
            let scripted = Scripted::start(vec![reply.clone(), second]).await;
            let id = with_history(&scripted).await;
            let settled = compact(&scripted.engine, &id).await;

            let bodies = scripted.model_bodies();
            assert_eq!(bodies.len(), 2, "one reminder, then no more");
            assert_ne!(bodies[1]["messages"], bodies[0]["messages"]);
            let messages = scripted.messages(&id).await;
            assert!(tool_states(&messages).is_empty(), "no tool ran");
            if recovers {
                assert!(settled.failures.is_empty(), "{:?}", settled.failures);
                assert_eq!(summaries(&messages), [sectioned_summary()]);
            } else {
                assert_eq!(settled.failures.len(), 1);
                assert!(summaries(&messages).is_empty());
            }
            assert!(user_texts(&messages).contains(&"Earlier question".to_owned()));
            scripted.finish().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_summary_cut_off_or_rate_limited_is_restarted_from_the_same_request() {
    let mut cut = text_turn("PARTIAL draft");
    cut.fault = Some(TurnFault::CloseAfterEvents(3));
    let scripted = Scripted::start(vec![
        cut,
        fault(TurnFault::Status(429)),
        text_turn("ACCEPTED summary"),
    ])
    .await;
    let id = with_history(&scripted).await;
    let settled = compact(&scripted.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(settled.retries.len(), 2, "{:?}", settled.retries);
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 3);
    assert_eq!(bodies[1], bodies[0]);
    assert_eq!(bodies[2], bodies[0]);
    let messages = scripted.messages(&id).await;
    assert_eq!(summaries(&messages), ["ACCEPTED summary"]);
    assert!(
        !messages
            .iter()
            .any(|message| texts(message).contains("PARTIAL draft")),
        "nothing of the discarded draft is stored"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_compaction_gives_up_after_a_bounded_number_of_retries_and_keeps_the_history() {
    let turns: Vec<Turn> = (0..10).map(|_| fault(TurnFault::Status(503))).collect();
    let scripted = Scripted::start(turns).await;
    let id = with_history(&scripted).await;
    let settled = compact(&scripted.engine, &id).await;

    assert_eq!(settled.failures.len(), 1);
    let attempts = 10 - scripted.instance.unconsumed_turns();
    assert!((2..10).contains(&attempts), "{attempts} attempts");
    let messages = scripted.messages(&id).await;
    assert!(summaries(&messages).is_empty());
    assert!(boundaries(&messages).is_empty());
    assert!(user_texts(&messages).contains(&"Earlier question".to_owned()));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a summary the provider refuses ends the compaction silently; no failure is reported"]
async fn a_summary_refused_by_the_provider_is_reported_as_a_failed_compaction() {
    let refused = Turn {
        stop: Some(StopReason::ContentFilter),
        ..Turn::default()
    };
    let scripted = Scripted::start(vec![refused]).await;
    let id = with_history(&scripted).await;
    let settled = compact(&scripted.engine, &id).await;

    assert_eq!(settled.failures.len(), 1);
    assert!(summaries(&scripted.messages(&id).await).is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a summary cut off by the output limit or a content filter is stored as the summary"]
async fn a_summary_that_did_not_finish_is_rejected_without_a_retry() {
    for stop in [StopReason::MaxTokens, StopReason::ContentFilter] {
        let mut unfinished = text_turn("INCOMPLETE summary");
        unfinished.stop = Some(stop);
        let scripted = Scripted::start(vec![unfinished, text_turn("Continued")]).await;
        let id = with_history(&scripted).await;
        let settled = compact(&scripted.engine, &id).await;
        assert_eq!(settled.failures.len(), 1, "{stop:?}");

        ask(
            &scripted.engine,
            scripted.prompt(&id, "Continue", Delivery::Steer),
        )
        .await;
        let bodies = scripted.model_bodies();
        assert_eq!(
            bodies.len(),
            2,
            "the unfinished summary is not asked for again"
        );
        let last = bodies[1].to_string();
        assert!(last.contains("Earlier question"));
        assert!(!last.contains("INCOMPLETE summary"));
        scripted.finish().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupting_a_manual_compaction_reports_no_failure_and_keeps_the_history() {
    let scripted = Scripted::start(vec![
        stalled(text_turn("Partial summary"), 1500),
        text_turn("Still here"),
    ])
    .await;
    let id = with_history(&scripted).await;
    let mut changes = scripted.engine.store().subscribe();
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    compaction_started(&mut changes).await;
    assert!(scripted.engine.abort(&id));
    let settled = settle(&mut events, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(scripted.engine.status(&id), RunStatus::Idle);
    let messages = scripted.messages(&id).await;
    assert!(summaries(&messages).is_empty());
    assert!(boundaries(&messages).is_empty());
    ask(
        &scripted.engine,
        scripted.prompt(&id, "next", Delivery::Steer),
    )
    .await;
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("Earlier question"));
    scripted.finish().await;
}

fn retarget(store: &Store, message: &MessageId, model: &str) {
    let Some(Message::User(mut prompt)) = store.get_message(message).unwrap() else {
        panic!("the seeded prompt is missing");
    };
    model.clone_into(&mut prompt.model);
    store.upsert_message(&Message::User(prompt)).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_compaction_whose_model_is_no_longer_offered_fails_unsent_and_frees_the_session() {
    let scripted = Scripted::start(vec![text_turn("SUMMARY once the model is back")]).await;
    let id = scripted.session().await;
    let store = scripted.engine.store();
    let question = seed_user(store, &id, "Earlier question");
    seed_reply(store, &id, &question, "Earlier answer", 0);
    retarget(store, &question, "withdrawn-model");

    let settled = compact(&scripted.engine, &id).await;
    let kinds: Vec<ErrorKind> = settled
        .failures
        .iter()
        .map(|failure| failure.kind)
        .collect();
    assert_eq!(kinds, [ErrorKind::ModelNotOffered]);
    assert!(scripted.model_bodies().is_empty(), "no model call is made");
    assert_eq!(scripted.engine.status(&id), RunStatus::Idle);
    assert!(boundaries(&scripted.messages(&id).await).is_empty());

    retarget(store, &question, MODEL);
    let settled = compact(&scripted.engine, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        summaries(&scripted.messages(&id).await),
        ["SUMMARY once the model is back"]
    );
    scripted.finish().await;
}
