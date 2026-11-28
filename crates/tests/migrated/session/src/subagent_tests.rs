//! Subagents started through the task tool: a child session under the caller,
//! its report back to the parent, no nesting for built-in subagents, and
//! failures and interrupts of the child.

use serde_json::json;
use systemprompt_engine::run::Delivery;
use systemprompt_engine::store::ToolState;
use systemprompt_scripted_instance::TurnFault;

use crate::support::{Scripted, fault, settle, text_turn, tool_states, tool_turn};

fn task_turn(agent: &str) -> systemprompt_scripted_instance::Turn {
    tool_turn(
        "task1",
        "task",
        json!({"description": "look around", "prompt": "Find the notes file", "agent": agent}),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subagent_runs_in_a_child_session_and_its_final_text_returns_to_the_caller() {
    let scripted = Scripted::start(vec![
        task_turn("general"),
        text_turn("child found notes.txt"),
        text_turn("parent done"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    let children = scripted.engine.store().child_sessions(&id).unwrap();
    assert_eq!(children.len(), 1);
    let states = tool_states(&scripted.messages(&id).await);
    let Some(ToolState::Completed {
        output, metadata, ..
    }) = states.first()
    else {
        panic!("{states:?}");
    };
    assert!(output.contains("child found notes.txt"));
    assert_eq!(metadata["session_id"], children[0].id.to_string());
    assert!(
        scripted.model_bodies()[2]
            .to_string()
            .contains("child found notes.txt")
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_built_in_subagent_is_not_offered_the_task_tool() {
    let scripted = Scripted::start(vec![
        task_turn("general"),
        text_turn("child"),
        text_turn("parent"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    let bodies = scripted.model_bodies();
    let names = |index: usize| -> Vec<String> {
        bodies[index]["tools"]
            .as_array()
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|t| t["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert!(names(0).contains(&"task".to_owned()));
    assert!(!names(1).contains(&"task".to_owned()), "{:?}", names(1));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_subagent_is_reported_to_the_caller_and_the_caller_continues() {
    let scripted = Scripted::start(vec![
        task_turn("general"),
        fault(TurnFault::Status(400)),
        text_turn("parent handles it"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert_eq!(scripted.model_bodies().len(), 3);
    let states = tool_states(&scripted.messages(&id).await);
    assert_eq!(states.len(), 1, "{states:?}");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_subagent_is_refused_without_creating_a_child() {
    let scripted = Scripted::start(vec![task_turn("nobody"), text_turn("ok")]).await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Delegate", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert!(
        scripted
            .engine
            .store()
            .child_sessions(&id)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Error { .. }]
    ));
    scripted.finish().await;
}
