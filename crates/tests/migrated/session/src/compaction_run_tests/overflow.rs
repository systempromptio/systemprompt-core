//! A prompt that does not fit: compaction before a request that would cross
//! the context ceiling, and recovery after the provider reports an overflow.

use systemprompt_engine::run::{Admitted, Delivery, ErrorKind, Purpose, RunStatus};
use systemprompt_engine::store::MessageErrorKind;
use systemprompt_scripted_instance::{StopReason, Turn, TurnFault};

use super::rig::{Model, Rig, overflow_stream, text_stream};
use super::{ask, boundaries, compaction_started, last_reply, seed_exchange, summaries};
use crate::support::{
    Scripted, Setup, fault, purposes, settle, stalled, text_turn, texts, user_texts, with_usage,
};

/// A session with one finished exchange whose reply reported `used` prompt
/// tokens.
async fn history(scripted: &Scripted, used: u64) -> systemprompt_engine::store::SessionId {
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        used,
    );
    id
}

fn kinds(failures: &[systemprompt_engine::run::SessionFailure]) -> Vec<ErrorKind> {
    failures.iter().map(|failure| failure.kind).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_output_limit_as_large_as_the_window_does_not_force_a_compaction() {
    let rig = Rig::scripted(vec![text_turn("Continued")], Model::sized(262_144, 262_144)).await;
    let id = rig.session().await;
    seed_exchange(
        rig.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        9_500,
    );
    let settled = ask(&rig.engine, rig.prompt(&id, "Continue")).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(rig.purposes(), [Purpose::Step]);
    let bodies = rig.bodies();
    assert!(bodies[0].to_string().contains("Earlier question"));
    assert!(boundaries(&rig.messages(&id).await).is_empty());
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_stops_when_the_compaction_it_needs_fails() {
    let scripted = Scripted::with(
        vec![fault(TurnFault::Status(400)), text_turn("Must not run")],
        Setup {
            context_window: Some(10_000),
            ..Setup::default()
        },
    )
    .await;
    let id = history(&scripted, 9_500).await;
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "Recent exact request", Delivery::Steer),
    )
    .await;

    assert_eq!(settled.failures.len(), 1);
    assert_eq!(purposes(&scripted), [Purpose::Compaction]);
    assert_eq!(scripted.instance.unconsumed_turns(), 1, "no step follows");
    let messages = scripted.messages(&id).await;
    assert!(summaries(&messages).is_empty());
    assert!(user_texts(&messages).contains(&"Recent exact request".to_owned()));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_context_overflow_is_recovered_by_one_compaction_and_a_retry() {
    for window in [200_000, 0] {
        let scripted = Scripted::with(
            vec![
                fault(TurnFault::ContextOverflow),
                text_turn("SUMMARY after the overflow"),
                text_turn("Recovered"),
            ],
            Setup {
                context_window: Some(window),
                ..Setup::default()
            },
        )
        .await;
        let id = history(&scripted, 0).await;
        let settled = ask(
            &scripted.engine,
            scripted.prompt(&id, "Continue", Delivery::Steer),
        )
        .await;

        assert!(
            settled.failures.is_empty(),
            "window {window}: {:?}",
            settled.failures
        );
        assert_eq!(
            purposes(&scripted),
            [Purpose::Step, Purpose::Compaction, Purpose::Step]
        );
        let last = scripted.model_bodies().last().unwrap().to_string();
        assert!(last.contains("SUMMARY after the overflow") && last.contains("Continue"));
        assert!(!last.contains("Earlier question"));
        let messages = scripted.messages(&id).await;
        assert_eq!(boundaries(&messages), [true]);
        assert_eq!(texts(last_reply(&messages)), "Recovered");
        scripted.finish().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_overflow_reported_inside_the_stream_before_any_output_is_recovered_the_same_way() {
    let rig = Rig::canned(
        vec![
            overflow_stream(None),
            text_stream("SUMMARY after the overflow"),
            text_stream("Recovered"),
        ],
        Model::sized(200_000, 8_192),
    )
    .await;
    let id = rig.session().await;
    seed_exchange(
        rig.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    let settled = ask(&rig.engine, rig.prompt(&id, "Continue")).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        rig.purposes(),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let bodies = rig.bodies();
    assert_eq!(bodies.len(), 3);
    assert!(bodies[2].to_string().contains("SUMMARY after the overflow"));
    let messages = rig.messages(&id).await;
    assert_eq!(boundaries(&messages), [true]);
    assert_eq!(texts(last_reply(&messages)), "Recovered");
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_window_smaller_than_the_prompt_is_compacted_before_the_first_request() {
    let scripted = Scripted::with(
        vec![
            text_turn("SUMMARY for the tiny window"),
            text_turn("Recovered"),
        ],
        Setup {
            context_window: Some(1),
            ..Setup::default()
        },
    )
    .await;
    let id = history(&scripted, 0).await;
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "Continue", Delivery::Steer),
    )
    .await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(purposes(&scripted), [Purpose::Compaction, Purpose::Step]);
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(
        bodies[1]
            .to_string()
            .contains("SUMMARY for the tiny window")
    );
    assert_eq!(
        texts(last_reply(&scripted.messages(&id).await)),
        "Recovered"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_overflow_after_one_recovery_ends_the_turn_with_the_overflow() {
    let scripted = Scripted::start(vec![
        fault(TurnFault::ContextOverflow),
        text_turn("SUMMARY after the first overflow"),
        fault(TurnFault::ContextOverflow),
    ])
    .await;
    let id = history(&scripted, 0).await;
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "Continue", Delivery::Steer),
    )
    .await;

    assert_eq!(kinds(&settled.failures), [ErrorKind::ContextOverflow]);
    assert_eq!(scripted.instance.unconsumed_turns(), 0);
    assert_eq!(scripted.model_bodies().len(), 3, "one recovery, no second");
    let messages = scripted.messages(&id).await;
    assert_eq!(boundaries(&messages), [true]);
    let reply = last_reply(&messages).message.as_assistant().unwrap();
    assert_eq!(
        reply.error.as_ref().map(|error| error.kind),
        Some(MessageErrorKind::ContextOverflow)
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_recovery_summary_leaves_the_original_overflow_as_the_turns_failure() {
    let scripted = Scripted::start(vec![
        fault(TurnFault::ContextOverflow),
        fault(TurnFault::Status(400)),
    ])
    .await;
    let id = history(&scripted, 0).await;
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "Continue", Delivery::Steer),
    )
    .await;

    assert_eq!(kinds(&settled.failures), [ErrorKind::ContextOverflow]);
    assert_eq!(scripted.model_bodies().len(), 2);
    assert!(summaries(&scripted.messages(&id).await).is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupting_an_overflow_recovery_during_its_summary_ends_the_turn_quietly() {
    let scripted = Scripted::start(vec![
        fault(TurnFault::ContextOverflow),
        stalled(text_turn("SUMMARY never finished"), 1500),
    ])
    .await;
    let id = history(&scripted, 0).await;
    let mut changes = scripted.engine.store().subscribe();
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Continue", Delivery::Steer))
        .await
        .unwrap();
    compaction_started(&mut changes).await;
    assert!(scripted.engine.abort(&id));
    let settled = settle(&mut events, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(scripted.engine.status(&id), RunStatus::Idle);
    let messages = scripted.messages(&id).await;
    assert!(summaries(&messages).is_empty());
    assert!(boundaries(&messages).is_empty());
    assert!(user_texts(&messages).contains(&"Continue".to_owned()));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a steer sent during an overflow recovery compaction is delivered with the retried request, not in a step after it"]
async fn prompts_sent_during_an_overflow_recovery_wait_until_the_retry_has_answered() {
    let scripted = Scripted::start(vec![
        fault(TurnFault::ContextOverflow),
        stalled(text_turn("SUMMARY after the overflow"), 1500),
        text_turn("Recovered"),
        text_turn("Steer handled"),
        text_turn("Queue handled"),
    ])
    .await;
    let id = history(&scripted, 0).await;
    let mut changes = scripted.engine.store().subscribe();
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Continue", Delivery::Steer))
        .await
        .unwrap();
    compaction_started(&mut changes).await;
    let queued = scripted
        .engine
        .prompt(scripted.prompt(&id, "Queued during compaction", Delivery::Queue))
        .await
        .unwrap();
    let steered = scripted
        .engine
        .prompt(scripted.prompt(&id, "Steered during compaction", Delivery::Steer))
        .await
        .unwrap();
    assert_eq!((queued, steered), (Admitted::Queued, Admitted::Steered));
    settle(&mut events, &id).await;

    let bodies = scripted.model_bodies();
    let retry = bodies[2].to_string();
    assert!(retry.contains("SUMMARY after the overflow"));
    assert!(!retry.contains("Steered during compaction"));
    assert!(!retry.contains("Queued during compaction"));
    assert_eq!(bodies.len(), 5);
    let after_retry = bodies[3].to_string();
    assert!(after_retry.contains("Steered during compaction"));
    assert!(!after_retry.contains("Queued during compaction"));
    assert!(bodies[4].to_string().contains("Queued during compaction"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_overflow_after_stored_output_is_not_recovered() {
    let rig = Rig::canned(
        vec![
            overflow_stream(Some("Partial")),
            text_stream("SUMMARY that must not be asked for"),
            text_stream("Must not run"),
        ],
        Model::sized(200_000, 8_192),
    )
    .await;
    let id = rig.session().await;
    let settled = ask(&rig.engine, rig.prompt(&id, "Fail after output")).await;

    assert_eq!(rig.bodies().len(), 1, "no summary and no retry are sent");
    assert_eq!(kinds(&settled.failures), [ErrorKind::ContextOverflow]);
    let messages = rig.messages(&id).await;
    let reply = last_reply(&messages);
    assert_eq!(texts(reply), "Partial");
    assert_eq!(
        reply
            .message
            .as_assistant()
            .and_then(|assistant| assistant.error.as_ref())
            .map(|error| error.kind),
        Some(MessageErrorKind::ContextOverflow)
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an empty reply cut at the output limit with the window full ends the turn; no compaction follows"]
async fn an_empty_reply_that_filled_the_window_is_treated_as_an_overflow() {
    let cut = Turn {
        stop: Some(StopReason::MaxTokens),
        ..Turn::default()
    };
    let scripted = Scripted::start(vec![
        with_usage(cut, 199_994, 0),
        text_turn("SUMMARY of the large prompt"),
        text_turn("Done"),
    ])
    .await;
    let id = scripted.session().await;
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "Large prompt", Delivery::Steer),
    )
    .await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        purposes(&scripted),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let messages = scripted.messages(&id).await;
    assert_eq!(summaries(&messages), ["SUMMARY of the large prompt"]);
    assert_eq!(texts(last_reply(&messages)), "Done");
    scripted.finish().await;
}
