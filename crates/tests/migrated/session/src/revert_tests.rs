//! Reverting to a prompt and bringing it back: what the next request sees,
//! step-by-step undo, redo of everything at once, and child sessions.

use systemprompt_engine::run::{CommandInput, CommandOutcome, Delivery};
use systemprompt_engine::store::SessionId;

use crate::support::{Scripted, settle, text_turn};

async fn three_prompts(scripted: &Scripted) -> SessionId {
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    for text in ["alpha-prompt", "beta-prompt", "gamma-prompt"] {
        scripted
            .engine
            .prompt(scripted.prompt(&id, text, Delivery::Steer))
            .await
            .unwrap();
        settle(&mut events, &id).await;
    }
    id
}

fn command(scripted: &Scripted, id: &SessionId, name: &str) -> CommandInput {
    CommandInput {
        session_id: id.clone(),
        directory: scripted.project.clone(),
        name: name.to_owned(),
        arguments: String::new(),
        agent: None,
        model: None,
        delivery: Delivery::Steer,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn reverting_to_a_prompt_hides_it_and_everything_after_from_the_next_request() {
    let scripted = Scripted::start(vec![
        text_turn("a1"),
        text_turn("a2"),
        text_turn("a3"),
        text_turn("a4"),
    ])
    .await;
    let id = three_prompts(&scripted).await;
    let messages = scripted.messages(&id).await;
    let second = messages
        .iter()
        .filter(|m| m.message.as_user().is_some())
        .nth(1)
        .unwrap()
        .message
        .id()
        .clone();
    let session = scripted.engine.revert(&id, &second).await.unwrap();
    assert_eq!(
        session.revert.as_ref().map(|marker| &marker.message_id),
        Some(&second)
    );
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "delta-prompt", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let last = scripted.model_bodies().last().unwrap().to_string();
    assert!(last.contains("alpha-prompt") && last.contains("delta-prompt"));
    assert!(!last.contains("beta-prompt") && !last.contains("gamma-prompt"));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_steps_back_one_prompt_at_a_time_and_redo_restores_them_all_at_once() {
    let scripted = Scripted::start(vec![text_turn("a1"), text_turn("a2"), text_turn("a3")]).await;
    let id = three_prompts(&scripted).await;
    let users: Vec<_> = scripted
        .messages(&id)
        .await
        .iter()
        .filter(|m| m.message.as_user().is_some())
        .map(|m| m.message.id().clone())
        .collect();
    let CommandOutcome::Reverted(first) = scripted
        .engine
        .command(command(&scripted, &id, "undo"))
        .await
        .unwrap()
    else {
        panic!("undo did not revert");
    };
    assert_eq!(
        first.revert.as_ref().map(|m| &m.message_id),
        Some(&users[2])
    );
    let CommandOutcome::Reverted(second) = scripted
        .engine
        .command(command(&scripted, &id, "undo"))
        .await
        .unwrap()
    else {
        panic!("undo did not revert");
    };
    assert_eq!(
        second.revert.as_ref().map(|m| &m.message_id),
        Some(&users[1])
    );
    let CommandOutcome::Reverted(redone) = scripted
        .engine
        .command(command(&scripted, &id, "redo"))
        .await
        .unwrap()
    else {
        panic!("redo did not run");
    };
    assert!(redone.revert.is_none());
    assert_eq!(scripted.messages(&id).await.len(), 6);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_after_a_revert_commits_it_and_the_reverted_messages_are_gone() {
    let scripted = Scripted::start(vec![
        text_turn("a1"),
        text_turn("a2"),
        text_turn("a3"),
        text_turn("a4"),
    ])
    .await;
    let id = three_prompts(&scripted).await;
    scripted
        .engine
        .command(command(&scripted, &id, "undo"))
        .await
        .unwrap();
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "replacement", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let users = crate::support::user_texts(&scripted.messages(&id).await);
    assert_eq!(users, ["alpha-prompt", "beta-prompt", "replacement"]);
    assert!(
        scripted
            .engine
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .revert
            .is_none()
    );
    scripted.finish().await;
}
