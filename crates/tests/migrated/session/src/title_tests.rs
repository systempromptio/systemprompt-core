//! Session titles generated from the first prompt: while the first step runs,
//! never over an existing or manual title, and what a failed attempt leaves.

use systemprompt_engine::run::{Delivery, Purpose};
use systemprompt_scripted_instance::TurnFault;

use crate::support::{Scripted, Setup, fault, purposes, settle, stalled, text_turn};

fn titled_first() -> Setup {
    Setup {
        title_first: true,
        ..Setup::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_title_is_generated_from_the_first_prompt_while_the_first_step_runs() {
    let scripted = Scripted::with(
        vec![text_turn("Greeting the user"), text_turn("Hello!")],
        titled_first(),
    )
    .await;
    let id = scripted.untitled().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Say hello to me", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    for _ in 0..100 {
        if !scripted
            .engine
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .title
            .is_empty()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        scripted.engine.get(&id).await.unwrap().unwrap().title,
        "Greeting the user"
    );
    assert_eq!(purposes(&scripted), [Purpose::Title, Purpose::Step]);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_existing_title_is_not_replaced() {
    let scripted = Scripted::start(vec![text_turn("Hello!")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Say hello", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert_eq!(purposes(&scripted), [Purpose::Step]);
    assert_eq!(
        scripted.engine.get(&id).await.unwrap().unwrap().title,
        "test session"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_rename_made_while_the_title_is_generated_is_kept() {
    let scripted = Scripted::with(
        vec![stalled(text_turn("Generated"), 1500), text_turn("Hi")],
        titled_first(),
    )
    .await;
    let id = scripted.untitled().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Say hi", Delivery::Steer))
        .await
        .unwrap();
    scripted
        .instance
        .wait_for(systemprompt_scripted_instance::Event::ModelCall)
        .await
        .unwrap();
    scripted
        .engine
        .store()
        .set_title(&id, "My own name")
        .unwrap();
    settle(&mut events, &id).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        scripted.engine.get(&id).await.unwrap().unwrap().title,
        "My own name"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a failed title call settles on the first line of the prompt and is not tried again"]
async fn a_failed_title_attempt_is_retried_from_the_first_prompt() {
    let scripted = Scripted::with(
        vec![
            fault(TurnFault::Status(400)),
            text_turn("One"),
            text_turn("Recovered title"),
            text_turn("Two"),
        ],
        titled_first(),
    )
    .await;
    let id = scripted.untitled().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "first prompt", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    scripted
        .engine
        .prompt(scripted.prompt(&id, "second prompt", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        scripted.engine.get(&id).await.unwrap().unwrap().title,
        "Recovered title"
    );
    scripted.finish().await;
}
