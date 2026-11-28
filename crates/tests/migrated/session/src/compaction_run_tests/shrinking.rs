//! Summary input that does not fit: what is resent after the provider rejects
//! it, and how a history larger than the window is cut before it is sent.

use serde_json::Value;
use systemprompt_engine::run::Delivery;
use systemprompt_engine::store::{SessionId, Store};
use systemprompt_scripted_instance::{Turn, TurnFault};

use super::rig::{Model, Rig};
use super::{
    IMAGE_DATA, ask, compact, image_part, only_user_turns, put_part, seed_exchange, seed_reply,
    seed_tool_step, seed_user, shrinking, sizes, summaries,
};
use crate::support::{Scripted, Setup, fault, text_turn};

fn filler() -> String {
    "context ".repeat(1_000)
}

/// Seeds `count` exchanges, each prompt carrying `body`.
fn fill(store: &Store, session: &SessionId, count: usize, body: &str) {
    for index in 0..count {
        seed_exchange(
            store,
            session,
            &format!("Request {index}: {body}"),
            &format!("Answer {index}"),
            0,
        );
    }
}

/// The estimate the engine budgets with: four characters to a token.
fn estimated_tokens(body: &Value) -> usize {
    body["messages"].to_string().len() / 4
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a summary request the provider rejects as too long fails the compaction at once; it is not resent smaller or as text"]
async fn a_summary_rejected_as_too_long_is_resent_smaller_four_times_before_failing() {
    let mut turns: Vec<Turn> = (0..5).map(|_| fault(TurnFault::ContextOverflow)).collect();
    turns.push(text_turn("Continued"));
    let scripted = Scripted::with(
        turns,
        Setup {
            context_window: Some(0),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    fill(scripted.engine.store(), &id, 8, &filler());
    let oldest = format!("Request 0: {}", filler());
    let settled = compact(&scripted.engine, &id).await;

    assert_eq!(settled.failures.len(), 1);
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 5, "four smaller inputs after the first");
    assert!(!bodies[2].to_string().contains(&oldest));
    assert!(shrinking(&sizes(&bodies[2..])), "{:?}", sizes(&bodies));

    ask(
        &scripted.engine,
        scripted.prompt(&id, "Continue", Delivery::Steer),
    )
    .await;
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains(&oldest), "the history is left as it was");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a summary request the provider rejects as too long fails the compaction at once; it is not resent smaller or as text"]
async fn a_summary_rejected_as_too_large_is_resent_as_text_without_media_then_smaller() {
    let turns: Vec<Turn> = (0..6).map(|_| fault(TurnFault::Status(413))).collect();
    let model = Model {
        window: 0,
        max_output: 8_192,
        images: true,
    };
    let rig = Rig::scripted(turns, model).await;
    let id = rig.session().await;
    let store = rig.engine.store();
    let pictured = seed_user(store, &id, "A request with a picture");
    put_part(store, &id, &pictured, image_part());
    seed_reply(store, &id, &pictured, "An answer about the picture", 0);
    fill(store, &id, 6, &filler());
    let settled = compact(&rig.engine, &id).await;

    assert_eq!(settled.failures.len(), 1);
    let bodies = rig.bodies();
    assert_eq!(bodies.len(), 6);
    assert!(bodies[0].to_string().contains(IMAGE_DATA));
    assert!(
        only_user_turns(&bodies[1]),
        "the resend is one text transcript"
    );
    assert!(!bodies[1].to_string().contains(IMAGE_DATA));
    assert!(shrinking(&sizes(&bodies[1..])), "{:?}", sizes(&bodies));
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a summary request the provider rejects as too long fails the compaction at once; it is not resent smaller or as text"]
async fn a_summary_already_sent_as_text_is_cut_further_after_a_too_large_rejection() {
    let turns = vec![
        fault(TurnFault::Status(413)),
        text_turn("SUMMARY after shrinking"),
    ];
    let rig = Rig::scripted(turns, Model::sized(12_000, 1_000)).await;
    let id = rig.session().await;
    fill(rig.engine.store(), &id, 6, &"x".repeat(8_000));
    let settled = compact(&rig.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let bodies = rig.bodies();
    assert_eq!(bodies.len(), 2);
    assert!(only_user_turns(&bodies[0]), "too big to send as turns");
    assert!(shrinking(&sizes(&bodies)), "{:?}", sizes(&bodies));
    assert_eq!(
        summaries(&rig.messages(&id).await),
        ["SUMMARY after shrinking"]
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the recovery summary request resends the rejected history whole, as the same conversation turns"]
async fn a_rejected_conversation_is_summarised_from_a_shortened_text_transcript() {
    let scripted = Scripted::start(vec![
        fault(TurnFault::ContextOverflow),
        text_turn("SUMMARY of the long history"),
        text_turn("Recovered"),
    ])
    .await;
    let id = scripted.session().await;
    fill(scripted.engine.store(), &id, 4, &filler());
    let settled = ask(
        &scripted.engine,
        scripted.prompt(&id, "Continue", Delivery::Steer),
    )
    .await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 3);
    let summary = &bodies[1];
    assert!(
        summary
            .get("tools")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    );
    assert!(
        only_user_turns(summary),
        "the rejected turns are not sent again as they were"
    );
    let sent = summary["messages"].to_string();
    assert!(!sent.contains(&format!("Request 0: {}", filler())));
    assert!(sent.contains("Request 3:"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a summary request the provider rejects as too long fails the compaction at once; it is not resent smaller or as text"]
async fn a_summary_rejected_as_too_long_is_resent_as_text_with_tool_results_and_no_media() {
    let turns = vec![
        fault(TurnFault::ContextOverflow),
        text_turn("SUMMARY from the transcript"),
    ];
    let model = Model {
        window: 200_000,
        max_output: 8_192,
        images: true,
    };
    let rig = Rig::scripted(turns, model).await;
    let id = rig.session().await;
    let store = rig.engine.store();
    let question = seed_user(store, &id, "Earlier question");
    put_part(store, &id, &question, image_part());
    let output = "x".repeat(5_000);
    seed_tool_step(store, &id, &question, 0, std::slice::from_ref(&output));
    seed_reply(store, &id, &question, "Earlier answer", 0);
    let settled = compact(&rig.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let bodies = rig.bodies();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[0].to_string().contains(IMAGE_DATA));
    let resent = bodies[1].to_string();
    assert!(only_user_turns(&bodies[1]));
    assert!(!resent.contains(IMAGE_DATA));
    assert!(resent.contains(&output), "tool output that fits is kept");
    assert_eq!(bodies[1]["system"], bodies[0]["system"]);
    assert_eq!(
        summaries(&rig.messages(&id).await),
        ["SUMMARY from the transcript"]
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a manual compaction sends the whole history whatever the window; nothing is cut to fit"]
async fn tool_output_is_dropped_from_the_summary_input_before_any_exchange_is() {
    let turns = vec![text_turn("SUMMARY within the window")];
    let rig = Rig::scripted(turns, Model::sized(7_000, 1_000)).await;
    let id = rig.session().await;
    let store = rig.engine.store();
    for index in 0..2 {
        let question = seed_user(store, &id, &format!("Request {index}"));
        let outputs = vec!["x".repeat(2_000); 12];
        seed_tool_step(store, &id, &question, index, &outputs);
        seed_reply(store, &id, &question, &format!("Answer {index}"), 0);
    }
    let settled = compact(&rig.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let bodies = rig.bodies();
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].to_string().contains("Request 0"));
    assert!(
        estimated_tokens(&bodies[0]) <= 7_000,
        "{} estimated tokens",
        estimated_tokens(&bodies[0])
    );
    rig.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a manual compaction sends the whole history whatever the window; nothing is cut to fit"]
async fn the_oldest_exchanges_are_left_out_of_a_summary_input_that_cannot_fit() {
    let turns = vec![text_turn("SUMMARY within the window")];
    let rig = Rig::scripted(turns, Model::sized(7_000, 1_000)).await;
    let id = rig.session().await;
    fill(rig.engine.store(), &id, 16, &"x".repeat(8_000));
    let settled = compact(&rig.engine, &id).await;

    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let bodies = rig.bodies();
    assert_eq!(bodies.len(), 1);
    let sent = bodies[0].to_string();
    assert!(
        !sent.contains("Request 0:"),
        "the oldest exchange is left out"
    );
    assert!(
        !sent.contains("Request 15:"),
        "the newest is kept verbatim instead"
    );
    assert!(estimated_tokens(&bodies[0]) <= 7_000);
    assert_eq!(
        summaries(&rig.messages(&id).await),
        ["SUMMARY within the window"]
    );
    rig.finish().await;
}
