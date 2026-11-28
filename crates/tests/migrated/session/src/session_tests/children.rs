//! Child sessions started through the task tool: where they live and what
//! they are called, whether they can be reverted, the model they run on, and
//! what an interrupt of the caller does to them.

use serde_json::json;
use systemprompt_engine::run::{Delivery, Purpose, RunStatus};
use systemprompt_engine::store::{Change, Message, MessageErrorKind, ToolState};

use super::helpers::{next_change, run, run_in, task_turn, undo};
use crate::support::{Scripted, purposes, settle, stalled, text_turn, tool_states, tool_turn};

fn delegating() -> Vec<systemprompt_scripted_instance::Turn> {
    vec![
        task_turn(),
        text_turn("child found notes.txt"),
        text_turn("parent done"),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn a_child_session_takes_its_callers_directory_and_project_and_its_tasks_name() {
    let scripted = Scripted::start(delegating()).await;
    let sub = scripted.project.join("packages");
    std::fs::create_dir_all(&sub).unwrap();
    let parent = scripted.engine.create(&sub).await.unwrap();
    scripted
        .engine
        .store()
        .set_title(&parent.id, "parent")
        .unwrap();
    run_in(&scripted, &parent.id, &sub, "Delegate").await;
    let children = scripted.engine.store().child_sessions(&parent.id).unwrap();
    let [child] = children.as_slice() else {
        panic!("{} child sessions", children.len());
    };
    assert_eq!(child.parent_id.as_ref(), Some(&parent.id));
    assert_eq!(child.directory, sub);
    assert_eq!(child.project_id, parent.project_id);
    assert_eq!(child.title, "look around");
    assert_eq!(
        purposes(&scripted),
        [Purpose::Step; 3],
        "no title was asked for"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a child session can be reverted like any other; nothing refuses it"]
async fn a_child_session_cannot_be_reverted() {
    let scripted = Scripted::start(delegating()).await;
    let id = scripted.session().await;
    run(&scripted, &id, "Delegate").await;
    let child = scripted.engine.store().child_sessions(&id).unwrap()[0]
        .id
        .clone();
    let outcome = scripted.engine.command(undo(&scripted, &child)).await;
    assert!(outcome.is_err(), "{outcome:?}");
    let read = scripted.engine.get(&child).await.unwrap().unwrap();
    assert!(read.revert.is_none());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the task tool takes no model: a model named in the call is ignored and the child runs on its agent's model or its caller's"]
async fn a_subagent_call_naming_a_model_that_is_not_offered_is_refused() {
    let call = json!({
        "description": "review",
        "prompt": "Review this",
        "agent": "general",
        "model": "unlisted-model"
    });
    let scripted = Scripted::start(vec![
        tool_turn("task1", "task", call),
        text_turn("answered by whoever runs next"),
        text_turn("parent done"),
    ])
    .await;
    let id = scripted.session().await;
    run(&scripted, &id, "Delegate the review").await;
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(
            states.as_slice(),
            [ToolState::Error { error, .. }] if error.contains("unlisted-model")
        ),
        "{states:?}"
    );
    assert!(
        scripted
            .engine
            .store()
            .child_sessions(&id)
            .unwrap()
            .is_empty()
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupting_the_caller_interrupts_its_running_subagent() {
    let scripted = Scripted::start(vec![
        task_turn(),
        stalled(text_turn("child still working"), 20_000),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    let mut feed = scripted.engine.store().subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate", Delivery::Steer))
        .await
        .unwrap();
    next_change(&mut feed, |change| {
        matches!(change, Change::MessageUpdated(Message::Assistant(reply)) if reply.session_id != id)
    })
    .await;
    assert!(scripted.engine.abort(&id));
    let settled = settle(&mut events, &id).await;
    assert!(
        settled.failures.is_empty(),
        "an interrupt is not reported as a failure"
    );
    let child = scripted.engine.store().child_sessions(&id).unwrap()[0]
        .id
        .clone();
    assert_eq!(scripted.engine.status(&child), RunStatus::Idle);
    let replies = scripted.messages(&child).await;
    let reply = replies
        .iter()
        .rev()
        .find_map(|message| message.message.as_assistant())
        .unwrap();
    assert_eq!(
        reply.error.as_ref().map(|error| error.kind),
        Some(MessageErrorKind::Aborted)
    );
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        matches!(states.as_slice(), [state] if state.is_finished()),
        "{states:?}"
    );
    assert!(
        scripted.model_bodies().len() <= 2,
        "the caller did not go on"
    );
    scripted.finish().await;
}
