//! Compaction inside a session: manual requests, the automatic trigger near
//! the context ceiling, a pending revert committed first, concurrent
//! requests, and failures that leave the conversation intact.

use systemprompt_engine::run::{Delivery, Purpose};
use systemprompt_engine::store::PartBody;
use systemprompt_scripted_instance::TurnFault;

use crate::support::{Scripted, Setup, fault, purposes, settle, text_turn, user_texts, with_usage};

async fn one_exchange(scripted: &Scripted, text: &str) -> systemprompt_engine::store::SessionId {
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, text, Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    id
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_compaction_replaces_older_history_with_a_summary_in_the_next_request() {
    let scripted = Scripted::start(vec![
        text_turn("first answer"),
        text_turn("SUMMARY: the user said hello"),
        text_turn("after compaction"),
    ])
    .await;
    let id = one_exchange(&scripted, "the original question").await;
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    settle(&mut events, &id).await;
    scripted
        .engine
        .prompt(scripted.prompt(&id, "next question", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;

    assert_eq!(
        purposes(&scripted),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let bodies = scripted.model_bodies();
    let last = bodies.last().unwrap().to_string();
    assert!(last.contains("SUMMARY: the user said hello"));
    assert!(!last.contains("the original question"));
    let messages = scripted.messages(&id).await;
    assert!(
        messages
            .iter()
            .flat_map(|m| m.parts.iter())
            .any(|part| matches!(part.body, PartBody::Compaction { auto: false }))
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn compacting_a_session_with_no_history_is_refused_without_calling_the_model() {
    let scripted = Scripted::start(vec![text_turn("unused")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    let started = scripted.engine.compact(&id).await;
    if started.is_ok() {
        let settled = settle(&mut events, &id).await;
        assert_eq!(settled.failures.len(), 1, "the refusal is explained");
    }
    assert!(scripted.model_bodies().is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn compaction_requests_made_together_result_in_one_compaction() {
    let mut summary = text_turn("SUMMARY");
    summary.fault = Some(TurnFault::StallMs(800));
    let scripted = Scripted::start(vec![text_turn("answer"), summary, text_turn("spare")]).await;
    let id = one_exchange(&scripted, "question").await;
    let mut events = scripted.engine.subscribe();
    let first = scripted.engine.compact(&id).await;
    let second = scripted.engine.compact(&id).await;
    assert!(first.is_ok());
    assert!(
        second.is_err(),
        "a second request while one runs is refused as busy"
    );
    settle(&mut events, &id).await;
    let compactions = purposes(&scripted)
        .iter()
        .filter(|p| **p == Purpose::Compaction)
        .count();
    assert_eq!(compactions, 1);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_compaction_leaves_the_conversation_intact_and_later_prompts_run() {
    let scripted = Scripted::start(vec![
        text_turn("first answer"),
        fault(TurnFault::Status(400)),
        text_turn("still here"),
    ])
    .await;
    let id = one_exchange(&scripted, "keep me").await;
    let before = scripted.messages(&id).await.len();
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    let settled = settle(&mut events, &id).await;
    assert_eq!(settled.failures.len(), 1);
    assert_eq!(scripted.messages(&id).await.len(), before);
    scripted
        .engine
        .prompt(scripted.prompt(&id, "next", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("keep me"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn compaction_starts_on_its_own_near_the_context_ceiling_and_keeps_the_latest_prompt() {
    let scripted = Scripted::with(
        vec![
            with_usage(text_turn("big answer"), 9_500, 100),
            text_turn("SUMMARY of the big exchange"),
            text_turn("answer after auto compaction"),
        ],
        Setup {
            context_window: Some(10_000),
            ..Setup::default()
        },
    )
    .await;
    let id = one_exchange(&scripted, "large first question").await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "small follow-up", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert_eq!(
        purposes(&scripted),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("SUMMARY of the big exchange"));
    assert!(last.contains("small follow-up"));
    assert!(!last.contains("large first question"));
    let messages = scripted.messages(&id).await;
    assert!(
        messages
            .iter()
            .flat_map(|m| m.parts.iter())
            .any(|part| matches!(part.body, PartBody::Compaction { auto: true }))
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn automatic_compaction_can_be_switched_off() {
    let scripted = Scripted::with(
        vec![
            with_usage(text_turn("big answer"), 9_500, 100),
            text_turn("no compaction"),
        ],
        Setup {
            context_window: Some(10_000),
            settings: Some(r#"{"compaction": {"auto": false}}"#.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    let id = one_exchange(&scripted, "large").await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "follow-up", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert_eq!(purposes(&scripted), [Purpose::Step, Purpose::Step]);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a manual compaction leaves a pending revert uncommitted"]
async fn a_pending_revert_is_committed_before_a_compaction_is_accepted() {
    let scripted = Scripted::start(vec![
        text_turn("answer one"),
        text_turn("answer two"),
        text_turn("SUMMARY"),
    ])
    .await;
    let id = one_exchange(&scripted, "keep this").await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "drop this", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let messages = scripted.messages(&id).await;
    let target = messages
        .iter()
        .filter(|m| m.message.as_user().is_some())
        .nth(1)
        .unwrap()
        .message
        .id()
        .clone();
    scripted.engine.revert(&id, &target).await.unwrap();
    scripted.engine.compact(&id).await.unwrap();
    settle(&mut events, &id).await;
    let users = user_texts(&scripted.messages(&id).await);
    assert!(!users.contains(&"drop this".to_owned()), "{users:?}");
    let summary_request = scripted.model_bodies().last().unwrap().to_string();
    assert!(!summary_request.contains("drop this"));
    scripted.finish().await;
}
