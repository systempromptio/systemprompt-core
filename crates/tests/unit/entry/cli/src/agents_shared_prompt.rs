//! Unit tests for the agent-selection prompt seam in
//! `admin::agents::shared`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::agents::shared::{prompt_agent_selection, select_agent_from_names};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_models::ServicesConfig;

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn select_agent_from_names_empty_errors() {
    let prompter = ScriptedPrompter::default();
    let err = select_agent_from_names(&prompter, "Select agent", Vec::new()).unwrap_err();
    assert!(err.to_string().contains("No agents configured"));
}

#[test]
fn select_agent_from_names_returns_scripted_choice() {
    let prompter = ScriptedPrompter::new(["1"]);
    let chosen = select_agent_from_names(
        &prompter,
        "Select agent",
        names(&["alpha", "beta", "gamma"]),
    )
    .expect("selection should succeed");
    assert_eq!(chosen, "beta");
}

#[test]
fn select_agent_from_names_sorts_before_selection() {
    let prompter = ScriptedPrompter::new(["0"]);
    let chosen = select_agent_from_names(&prompter, "Select agent", names(&["zebra", "apple"]))
        .expect("selection should succeed");
    assert_eq!(chosen, "apple");
}

#[test]
fn select_agent_from_names_rejects_out_of_range_index() {
    let prompter = ScriptedPrompter::new(["5"]);
    let err = select_agent_from_names(&prompter, "Select agent", names(&["only"])).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn select_agent_from_names_errors_when_prompter_exhausted() {
    let prompter = ScriptedPrompter::default();
    let err = select_agent_from_names(&prompter, "Select agent", names(&["a", "b"])).unwrap_err();
    assert!(err.to_string().contains("exhausted"));
}

#[test]
fn prompt_agent_selection_empty_config_errors() {
    let prompter = ScriptedPrompter::default();
    let config = ServicesConfig::default();
    let err = prompt_agent_selection(&prompter, "Select agent", &config).unwrap_err();
    assert!(err.to_string().contains("No agents configured"));
}
