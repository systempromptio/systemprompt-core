use std::sync::Arc;

use harness_config::permission;
use harness_engine::Delivery;
use harness_prompt::tool_names;
use harness_tools::names;

use crate::support::{
    Scripted, TestGovernor, harness_with, prompt, text_reply, titled_session,
    tool_names as offered, wait_idle,
};

#[test]
fn tool_names_agree_across_crates() {
    assert_eq!(names::SHELL, tool_names::SHELL);
    assert_eq!(names::READ, tool_names::READ);
    assert_eq!(names::WRITE, tool_names::WRITE);
    assert_eq!(names::EDIT, tool_names::EDIT);
    assert_eq!(names::PATCH, tool_names::PATCH);
    assert_eq!(names::GLOB, tool_names::GLOB);
    assert_eq!(names::GREP, tool_names::GREP);
    assert_eq!(names::FETCH, tool_names::FETCH);
    assert_eq!(names::TODO, tool_names::TODO);
    assert_eq!(permission::TASK, tool_names::TASK);
    assert_eq!(permission::SKILL, tool_names::SKILL);
    assert_eq!(permission::QUESTION, tool_names::QUESTION);
}

#[tokio::test]
async fn denying_edit_withholds_every_file_writer() {
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("Fine."))]));
    let settings = r#"{"permission": {"edit": "deny"}}"#;
    let harness = harness_with(
        Arc::new(TestGovernor::default()),
        Arc::clone(&transport),
        Some(settings),
    );
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Hello", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;

    let tools = offered(&transport.requests()[0]);
    assert!(tools.iter().any(|tool| tool == names::READ), "{tools:?}");
    for writer in [names::WRITE, names::EDIT, names::PATCH] {
        assert!(
            !tools.iter().any(|tool| tool == writer),
            "{writer} is offered: {tools:?}"
        );
    }
    harness.engine.shutdown().await;
}
