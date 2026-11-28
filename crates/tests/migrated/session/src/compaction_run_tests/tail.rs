//! The verbatim tail kept beside a summary: what goes into it, how a later
//! summary and a fork see it, and how media in it reaches the model.

use serde_json::json;
use systemprompt_engine::run::{Delivery, Purpose};
use systemprompt_engine::store::{MessageWithParts, PartBody, ToolState};

use super::rig::{Model, Rig};
use super::{ask, compact, seed_exchange, seed_tool_step, seed_user, summaries};
use crate::support::{Scripted, Setup, purposes, text_turn, tool_states, tool_turn, with_usage};

/// The start of a PNG file: enough for the read tool to attach it as an image.
const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
];

/// A scripted engine on a 10,000 token window whose session holds one
/// exchange that nearly filled it, so the next prompt starts a compaction.
async fn near_the_ceiling(turns: &[&str]) -> (Scripted, systemprompt_engine::store::SessionId) {
    let scripted = Scripted::with(
        turns.iter().copied().map(text_turn).collect(),
        Setup {
            context_window: Some(10_000),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    seed_exchange(
        scripted.engine.store(),
        &id,
        "large first question",
        "big answer",
        9_500,
    );
    (scripted, id)
}

fn attached_data(messages: &[MessageWithParts]) -> String {
    tool_states(messages)
        .into_iter()
        .find_map(|state| match state {
            ToolState::Completed { attachments, .. } => attachments
                .first()
                .and_then(|attachment| attachment.url.split_once(','))
                .map(|(_, data)| data.to_owned()),
            _ => None,
        })
        .expect("the read attached the picture")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the tail kept beside a summary is resent verbatim, so a tool result's image goes back to the model as image data"]
async fn media_in_the_kept_tail_is_described_not_resent() {
    let turns = vec![
        with_usage(
            tool_turn("t1", "read", json!({"path": "pixel.png"})),
            9_500,
            100,
        ),
        text_turn("SUMMARY before the picture"),
        text_turn("Seen"),
    ];
    let model = Model {
        window: 10_000,
        max_output: 8_192,
        images: true,
    };
    let rig = Rig::scripted(turns, model).await;
    std::fs::write(rig.project.join("pixel.png"), PNG).unwrap();
    let id = rig.session().await;
    seed_exchange(
        rig.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        0,
    );
    let settled = ask(&rig.engine, rig.prompt(&id, "Look at pixel.png")).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        rig.purposes(),
        [Purpose::Step, Purpose::Compaction, Purpose::Step]
    );
    let data = attached_data(&rig.messages(&id).await);
    let last = rig.bodies().last().unwrap().to_string();
    assert!(last.contains("pixel.png"), "the picture is still named");
    assert!(!last.contains(&data), "its bytes are not sent again");
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a fork copies the compaction mark with the source session's message ids, so the fork loses the verbatim tail"]
async fn a_fork_keeps_the_verbatim_tail_of_the_summary_it_copied() {
    let (scripted, id) = near_the_ceiling(&[
        "SUMMARY of the first exchange",
        "tail answer",
        "fork answer",
    ])
    .await;
    ask(
        &scripted.engine,
        scripted.prompt(&id, "kept follow-up", Delivery::Steer),
    )
    .await;
    let fork = scripted.engine.fork(&id, None).await.unwrap();
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&fork.id, "after the fork", Delivery::Steer),
    )
    .await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("SUMMARY of the first exchange"));
    assert!(
        last.contains("kept follow-up"),
        "the tail came with the fork"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unusable_tail_reference_does_not_break_history_or_forking() {
    let scripted = Scripted::start(vec![
        text_turn("SUMMARY kept"),
        text_turn("after the edit"),
        text_turn("fork answer"),
    ])
    .await;
    let id = scripted.session().await;
    let store = scripted.engine.store();
    seed_exchange(store, &id, "Earlier question", "Earlier answer", 0);
    compact(&scripted.engine, &id).await;
    let boundary = scripted
        .messages(&id)
        .await
        .iter()
        .find(|message| {
            message
                .parts
                .iter()
                .any(|part| matches!(part.body, PartBody::Compaction { .. }))
        })
        .expect("the compaction left its boundary")
        .message
        .id()
        .to_string();
    // JSON: the engine's own bookkeeping entry, written with a tail that names no
    // message.
    let mark = json!({"message": boundary, "tail": "not-a-message-id"});
    store
        .update_session(&id, |session| {
            session.metadata.insert("compaction".to_owned(), mark);
        })
        .unwrap();

    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "next question", Delivery::Steer),
    )
    .await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("SUMMARY kept") && !last.contains("Earlier question"));

    let fork = scripted.engine.fork(&id, None).await.unwrap();
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&fork.id, "in the fork", Delivery::Steer),
    )
    .await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("SUMMARY kept") && last.contains("in the fork"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_later_summary_is_asked_to_update_the_earlier_one_and_is_sent_the_kept_tail() {
    let (scripted, id) =
        near_the_ceiling(&["FIRST SUMMARY", "tail answer", "SECOND SUMMARY"]).await;
    ask(
        &scripted.engine,
        scripted.prompt(&id, "kept follow-up", Delivery::Steer),
    )
    .await;
    let settled = compact(&scripted.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(
        purposes(&scripted),
        [Purpose::Compaction, Purpose::Step, Purpose::Compaction]
    );
    let update = scripted.model_bodies().last().unwrap().to_string();
    assert!(update.contains("kept follow-up") && update.contains("tail answer"));
    assert!(update.contains("FIRST SUMMARY"));
    assert!(!update.contains("large first question"));
    assert_eq!(
        summaries(&scripted.messages(&id).await),
        ["FIRST SUMMARY", "SECOND SUMMARY"]
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a manual compaction condenses the whole history and keeps no verbatim tail"]
async fn a_manual_compaction_keeps_the_latest_exchange_verbatim_when_the_whole_cannot_fit() {
    let turns = vec![text_turn("SUMMARY of the older part"), text_turn("after")];
    let rig = Rig::scripted(turns, Model::sized(200_000, 32_000)).await;
    let id = rig.session().await;
    let store = rig.engine.store();
    let older = format!("older {}", "x".repeat(640_000));
    let latest = format!("latest {}", "y".repeat(120_000));
    seed_user(store, &id, &older);
    seed_user(store, &id, &latest);
    let settled = compact(&rig.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let summary = rig.bodies().remove(0).to_string();
    assert!(
        summary.contains(&older),
        "the part before the tail is sent whole"
    );
    assert!(
        !summary.contains(&"y".repeat(1_000)),
        "the tail is not condensed"
    );
    ask(&rig.engine, rig.prompt(&id, "next")).await;
    let last = rig.bodies().last().unwrap().to_string();
    assert!(
        last.contains(&latest),
        "the tail is still there word for word"
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a manual compaction condenses the whole history and keeps no verbatim tail"]
async fn a_long_run_of_tool_steps_keeps_only_its_newest_steps_verbatim() {
    let turns = vec![text_turn("SUMMARY of the run"), text_turn("after")];
    let rig = Rig::scripted(turns, Model::sized(200_000, 32_000)).await;
    let id = rig.session().await;
    let store = rig.engine.store();
    seed_user(store, &id, "Earlier question");
    let run = seed_user(store, &id, "Run the migration");
    for step in 0..60 {
        let output = format!("step-{step:02}-output {}", "x".repeat(1_250));
        seed_tool_step(store, &id, &run, step, &[output]);
    }
    let settled = compact(&rig.engine, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);

    ask(&rig.engine, rig.prompt(&id, "next")).await;
    let last = rig.bodies().last().unwrap().to_string();
    assert!(last.contains("step-59-output"), "the newest steps are kept");
    assert!(!last.contains("step-00-output"), "the oldest are condensed");
    assert!(
        !last.contains("Run the migration"),
        "the tail starts inside the run, after the prompt that began it"
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_newest_message_larger_than_the_tail_allowance_is_condensed_with_the_rest() {
    let (scripted, id) = near_the_ceiling(&["SUMMARY with the latest prompt", "after"]).await;
    let bulk = "x".repeat(20_000);
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, &format!("Latest {bulk}"), Delivery::Steer),
    )
    .await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(purposes(&scripted), [Purpose::Compaction, Purpose::Step]);
    let bodies = scripted.model_bodies();
    assert!(bodies[0].to_string().contains(&bulk), "it is summarised");
    let after = bodies[1].to_string();
    assert!(after.contains("SUMMARY with the latest prompt"));
    assert!(!after.contains(&bulk), "and not kept verbatim");
    scripted.finish().await;
}
