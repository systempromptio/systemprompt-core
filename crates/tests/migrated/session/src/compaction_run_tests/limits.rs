//! The output limit a request asks for: sized to the room the prompt leaves
//! for a step, and to the headroom reserved for it for a summary.

use serde_json::Value;
use systemprompt_engine::run::Purpose;
use systemprompt_engine::store::{SessionId, Store};

use super::rig::{Model, Rig};
use super::{ask, compact, max_tokens, seed_exchange, seed_summary, seed_user};
use crate::support::text_turn;

/// A rig whose session already holds one exchange that used `used` prompt
/// tokens.
async fn after_an_exchange(turns: &[&str], model: Model, used: u64) -> (Rig, SessionId) {
    let turns = turns.iter().copied().map(text_turn).collect();
    let rig = Rig::scripted(turns, model).await;
    let id = rig.session().await;
    seed_exchange(
        rig.engine.store(),
        &id,
        "Earlier question",
        "Earlier answer",
        used,
    );
    (rig, id)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a step asks for the model's whole output ceiling whatever room the prompt leaves in the window"]
async fn a_steps_output_limit_shrinks_to_the_room_the_prompt_leaves() {
    let (rig, id) = after_an_exchange(&["Continued"], Model::sized(100_000, 64_000), 50_000).await;
    ask(&rig.engine, rig.prompt(&id, "Continue")).await;

    let limit = max_tokens(&rig.bodies()[0]);
    assert!(
        (48_801..49_000).contains(&limit),
        "{limit}: the window less the prompt, with a hundredth of the window held back"
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a step asks for whatever output ceiling the model states; no upper cap is applied"]
async fn a_very_large_stated_output_limit_is_capped() {
    let turns = vec![text_turn("Answer")];
    let rig = Rig::scripted(turns, Model::sized(1_000_000, 1_000_000)).await;
    let id = rig.session().await;
    ask(&rig.engine, rig.prompt(&id, "Question")).await;

    assert_eq!(max_tokens(&rig.bodies()[0]), 256_000);
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the summary request asks for the model's whole output ceiling, not the headroom reserved for it"]
async fn the_summary_gets_the_reserved_headroom_and_the_next_step_the_models_ceiling() {
    let (rig, id) = after_an_exchange(
        &["SUMMARY that keeps the task", "Continued"],
        Model::sized(200_000, 32_000),
        185_000,
    )
    .await;
    let recent = "Recent request ".repeat(400);
    let settled = ask(&rig.engine, rig.prompt(&id, &recent)).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(rig.purposes(), [Purpose::Compaction, Purpose::Step]);
    let limits: Vec<u64> = rig.bodies().iter().map(max_tokens).collect();
    assert_eq!(limits, [30_000, 32_000]);
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn on_a_small_window_the_summary_keeps_its_whole_output_limit() {
    let (rig, id) = after_an_exchange(
        &["SUMMARY that keeps the task", "Continued"],
        Model::sized(64_000, 16_000),
        59_000,
    )
    .await;
    let recent = "Recent request ".repeat(400);
    let settled = ask(&rig.engine, rig.prompt(&id, &recent)).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert_eq!(rig.purposes(), [Purpose::Compaction, Purpose::Step]);
    let limits: Vec<u64> = rig.bodies().iter().map(max_tokens).collect();
    assert_eq!(limits, [16_000, 16_000]);
    rig.finish().await;
}

/// The summary request of a manual compaction over the history `seed`
/// writes, for a model with a 200,000 token window and a 32,000 token output
/// ceiling.
async fn summary_request(seed: impl FnOnce(&Store, &SessionId)) -> Value {
    let turns = vec![text_turn("SUMMARY")];
    let rig = Rig::scripted(turns, Model::sized(200_000, 32_000)).await;
    let id = rig.session().await;
    seed(rig.engine.store(), &id);
    let settled = compact(&rig.engine, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let request = rig.bodies().remove(0);
    rig.finish().await;
    request
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the summary request asks for the model's whole output ceiling whatever room its input leaves"]
async fn the_summary_requests_output_limit_follows_the_room_its_input_leaves() {
    let fitting = summary_request(|store, id| {
        seed_user(store, id, &"x".repeat(640_000));
    })
    .await;
    assert_eq!(
        max_tokens(&fitting),
        30_000,
        "input under the target leaves the whole reserve"
    );

    let first = format!("first {}", "x".repeat(364_000));
    let near_the_window = summary_request(|store, id| {
        seed_user(store, id, &first);
        seed_user(store, id, &format!("second {}", "x".repeat(364_000)));
    })
    .await;
    assert!(
        near_the_window.to_string().contains(&first),
        "a history that still fits the window is sent unchanged"
    );
    assert!(max_tokens(&near_the_window) < 20_000);

    let unshrinkable = summary_request(|store, id| {
        seed_summary(store, id, &"x".repeat(1_000_000));
        seed_user(store, id, "Next question");
    })
    .await;
    assert_eq!(
        max_tokens(&unshrinkable),
        1_024,
        "an input that cannot be cut leaves only the floor"
    );
}
