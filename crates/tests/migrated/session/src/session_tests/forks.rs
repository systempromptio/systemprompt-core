//! Forks: what a copy carries over (title, session rules), what it leaves
//! behind (unfinished replies, later prompts and the skills they loaded), and
//! what the model is told about it.

use std::path::Path;

use serde_json::json;
use systemprompt_engine::config::permission::READ;
use systemprompt_engine::config::{Action, Rule, Ruleset};
use systemprompt_engine::store::{
    AssistantMessage, CallId, JsonMap, MessageId, MessagePage, NewSession, Part, PartBody, Session,
    SessionId, Store, ToolState, UserMessage, now_ms,
};

use super::helpers::{add_skill, run, task_turn};
use crate::support::{Scripted, read_turn, text_turn, texts, tool_states, tool_turn, user_texts};

fn session(store: &Store) -> Session {
    let project = store
        .ensure_project(Path::new("/work/demo"), "demo")
        .unwrap();
    store
        .create_session(NewSession::new(project.id, "/work/demo"))
        .unwrap()
}

fn user(store: &Store, session: &SessionId, text: &str) -> MessageId {
    let message = UserMessage::new(store.ids(), session.clone(), "build", "model-a");
    let id = message.id.clone();
    store.upsert_message(&message.into()).unwrap();
    let body = PartBody::Text {
        text: text.to_owned(),
        synthetic: false,
    };
    store
        .upsert_part(&Part::new(store.ids(), session.clone(), id.clone(), body))
        .unwrap();
    id
}

#[test]
fn a_fork_takes_its_sources_title_and_stays_untitled_when_there_is_none() {
    let store = Store::open_in_memory().unwrap();
    let untitled = session(&store);
    user(&store, &untitled.id, "First");
    let fork = store.fork_session(&untitled.id, None).unwrap();
    assert!(fork.title.is_empty());
    assert!(
        store
            .get_session(&fork.id)
            .unwrap()
            .unwrap()
            .title
            .is_empty()
    );
    let titled = session(&store);
    store.set_title(&titled.id, "Named").unwrap();
    assert_eq!(store.fork_session(&titled.id, None).unwrap().title, "Named");
}

#[test]
#[ignore = "parity ledger: a fork copies a reply that is still running, with its unfinished tool call"]
fn a_reply_that_is_still_running_is_not_copied_into_a_fork() {
    let store = Store::open_in_memory().unwrap();
    let source = session(&store);
    let prompt = user(&store, &source.id, "Run both tools");
    let running = AssistantMessage::new(store.ids(), source.id.clone(), prompt, "build", "model-a");
    let running_id = running.id.clone();
    store.upsert_message(&running.into()).unwrap();
    let call = PartBody::Tool {
        call_id: CallId::try_new("call-running").unwrap(),
        tool: "shell".to_owned(),
        state: Box::new(ToolState::Running {
            input: json!({"command": "sleep 10"}),
            title: None,
            metadata: JsonMap::new(),
            started: now_ms(),
        }),
    };
    store
        .upsert_part(&Part::new(store.ids(), source.id.clone(), running_id, call))
        .unwrap();
    let fork = store.fork_session(&source.id, None).unwrap();
    assert_eq!(
        store
            .messages(&source.id, &MessagePage::all())
            .unwrap()
            .len(),
        2
    );
    let copied = store.messages(&fork.id, &MessagePage::all()).unwrap();
    assert_eq!(copied.len(), 1, "only the settled prompt is copied");
    assert!(copied[0].message.as_user().is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a child session starts without its parent's session rules; only a fork carries them"]
async fn session_rules_pass_to_forks_and_to_child_sessions() {
    let scripted = Scripted::start(vec![
        task_turn(),
        read_turn("r1"),
        text_turn("child done"),
        text_turn("parent done"),
    ])
    .await;
    let id = scripted.session().await;
    let rules = Ruleset::from(vec![Rule::new(READ, "*", Action::Deny)]);
    let stored = serde_json::to_value(&rules).unwrap();
    let expected = Some(stored.clone());
    scripted
        .engine
        .store()
        .update_session(&id, |session| session.permission = Some(stored))
        .unwrap();
    let fork = scripted.engine.fork(&id, None).await.unwrap();
    assert_eq!(fork.permission, expected);
    run(&scripted, &id, "Delegate the reading").await;
    let children = scripted.engine.store().child_sessions(&id).unwrap();
    let states = tool_states(&scripted.messages(&children[0].id).await);
    assert!(
        matches!(states.as_slice(), [ToolState::Error { .. }]),
        "the child read a file its parent's rules deny: {states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a fork holds only the copied history; no notice separates it from the first new prompt"]
async fn a_fork_tells_the_model_where_the_copied_history_ends() {
    let scripted =
        Scripted::start(vec![text_turn("first answer"), text_turn("second answer")]).await;
    let id = scripted.session().await;
    run(&scripted, &id, "Original").await;
    let fork = scripted.engine.fork(&id, None).await.unwrap();
    run(&scripted, &fork.id, "Continue").await;
    let messages = scripted.messages(&fork.id).await;
    let users: Vec<(bool, String)> = messages
        .iter()
        .filter_map(|message| {
            message
                .message
                .as_user()
                .map(|user| (user.system, texts(message)))
        })
        .collect();
    assert_eq!(users.len(), 3, "{users:?}");
    assert_eq!(users[0], (false, "Original".to_owned()));
    assert!(users[1].0 && !users[1].1.is_empty(), "{users:?}");
    assert_eq!(users[2], (false, "Continue".to_owned()));
    assert_eq!(user_texts(&scripted.messages(&id).await), ["Original"]);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fork_taken_before_a_prompt_leaves_out_the_skill_that_prompt_loaded() {
    let scripted = Scripted::start(vec![
        text_turn("ready"),
        tool_turn("s1", "skill", json!({"name": "deploy"})),
        text_turn("Following it."),
    ])
    .await;
    add_skill(&scripted);
    let id = scripted.session().await;
    run(&scripted, &id, "Before the skill").await;
    run(&scripted, &id, "Apply the deploy skill").await;
    let messages = scripted.messages(&id).await;
    assert!(
        matches!(
            tool_states(&messages).as_slice(),
            [ToolState::Completed { output, .. }] if output.contains("release checklist")
        ),
        "the source session loaded the skill"
    );
    let selected = messages
        .iter()
        .filter_map(|message| message.message.as_user())
        .nth(1)
        .unwrap()
        .id
        .clone();
    let fork = scripted.engine.fork(&id, Some(&selected)).await.unwrap();
    let copied = scripted.messages(&fork.id).await;
    assert_eq!(user_texts(&copied), ["Before the skill"]);
    assert!(tool_states(&copied).is_empty());
    assert!(
        copied
            .iter()
            .all(|message| !texts(message).contains("release checklist"))
    );
    scripted.finish().await;
}
