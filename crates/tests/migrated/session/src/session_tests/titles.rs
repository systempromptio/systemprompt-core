//! Titles beyond the first exchange: one request however many prompts follow,
//! and a title agent that was switched off.

use systemprompt_engine::run::Purpose;
use systemprompt_engine::store::Change;

use super::helpers::{next_change, run};
use crate::support::{Scripted, Setup, purposes, text_turn};

#[tokio::test(flavor = "multi_thread")]
async fn the_title_is_asked_for_once_from_the_first_prompt_however_many_follow() {
    let scripted = Scripted::with(
        vec![
            text_turn("Counting sheep"),
            text_turn("one"),
            text_turn("two"),
        ],
        Setup {
            title_first: true,
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.untitled().await;
    let mut feed = scripted.engine.store().subscribe();
    run(&scripted, &id, "First message about sheep").await;
    run(&scripted, &id, "Second message about goats").await;
    let titled = next_change(&mut feed, |change| {
        matches!(change, Change::SessionUpdated(session) if session.id == id && !session.title.is_empty())
    })
    .await;
    let Change::SessionUpdated(titled) = titled else {
        unreachable!()
    };
    assert_eq!(titled.title, "Counting sheep");
    assert_eq!(
        purposes(&scripted),
        [Purpose::Title, Purpose::Step, Purpose::Step]
    );
    let request = scripted.model_bodies()[0].to_string();
    assert!(request.contains("First message about sheep"));
    assert!(!request.contains("Second message about goats"));
    assert_eq!(
        scripted.engine.get(&id).await.unwrap().unwrap().title,
        "Counting sheep"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: disabling the title agent does not stop title generation; the built-in title prompt is used instead"]
async fn no_title_is_generated_when_the_title_agent_is_disabled() {
    let scripted = Scripted::with(
        vec![text_turn("A title nobody asked for"), text_turn("Hello!")],
        Setup {
            settings: Some(r#"{"agent": {"title": {"disable": true}}}"#.to_owned()),
            title_first: true,
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.untitled().await;
    run(&scripted, &id, "Help me debug the failing build").await;
    assert_eq!(purposes(&scripted), [Purpose::Step]);
    assert!(
        scripted
            .engine
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .title
            .is_empty()
    );
    scripted.finish().await;
}
