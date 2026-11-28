use std::sync::Arc;

use systemprompt_engine::config::permission;
use systemprompt_engine::prompt::tool_names;
use systemprompt_engine::run::Delivery;
use systemprompt_engine::tools::names;

use crate::run::support::{
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

fn add_skill(project: &std::path::Path, name: &str, description: &str) {
    let dir = project.join(".harness/skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let text = format!("---\nname: {name}\ndescription: {description}\n---\nFollow the steps.\n");
    std::fs::write(dir.join("SKILL.md"), text).unwrap();
}

async fn first_request(settings: &str) -> serde_json::Value {
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("Fine."))]));
    let harness = harness_with(
        Arc::new(TestGovernor::default()),
        Arc::clone(&transport),
        Some(settings),
    );
    add_skill(&harness.project, "deploy", "Ships a release");
    add_skill(&harness.project, "review", "Reviews a change");
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();
    harness
        .engine
        .prompt(prompt(&harness, &id, "Hello", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;
    harness.engine.shutdown().await;
    transport.requests()[0].clone()
}

#[tokio::test]
async fn a_skill_a_rule_denies_by_name_is_neither_listed_nor_loadable() {
    let settings = r#"{"permission": [
        {"permission": "skill", "pattern": "*", "action": "deny"},
        {"permission": "skill", "pattern": "*", "action": "allow"},
        {"permission": "skill", "pattern": "deploy", "action": "allow"},
        {"permission": "skill", "pattern": "deploy", "action": "deny"}
    ]}"#;
    let request = first_request(settings).await;
    let sent = request.to_string();
    assert!(!sent.contains("Ships a release"), "{sent}");
    assert!(sent.contains("Reviews a change"), "{sent}");
    let skill = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == tool_names::SKILL)
        .expect("the skill tool is offered");
    assert!(!skill.to_string().contains("deploy"), "{skill}");
}

#[tokio::test]
async fn the_skill_tool_is_withheld_when_every_skill_is_denied_by_name() {
    let settings = r#"{"permission": [
        {"permission": "skill", "pattern": "deploy", "action": "deny"},
        {"permission": "skill", "pattern": "review", "action": "deny"}
    ]}"#;
    let request = first_request(settings).await;
    let tools = offered(&request);
    assert!(
        !tools.iter().any(|tool| tool == tool_names::SKILL),
        "{tools:?}"
    );
    assert!(!request.to_string().contains("Reviews a change"));
}
