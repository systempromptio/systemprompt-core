//! Tests for agent log-file selection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::admin::agents::logs_disk::select_agent_from_log_files;

#[test]
fn selection_maps_the_chosen_index_back_to_an_agent_name() {
    let prompter = ScriptedPrompter::new(["1"]);
    let files = vec![
        "agent-alpha.log".to_owned(),
        "agent-beta.log".to_owned(),
        "agent-gamma.log".to_owned(),
    ];

    let chosen =
        select_agent_from_log_files(&prompter, &files, Path::new("/var/log/systemprompt")).unwrap();
    assert_eq!(chosen, "beta");
}

#[test]
fn an_empty_log_directory_names_the_directory_in_the_error() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    let err = select_agent_from_log_files(&prompter, &[], Path::new("/var/log/systemprompt"))
        .unwrap_err();
    assert!(err.to_string().contains("/var/log/systemprompt"));
    assert!(err.to_string().contains("No agent log files found"));
}

#[test]
fn an_out_of_range_selection_is_surfaced_as_an_error() {
    let prompter = ScriptedPrompter::new(["7"]);
    let files = vec!["agent-alpha.log".to_owned()];

    let err = select_agent_from_log_files(&prompter, &files, Path::new("/tmp")).unwrap_err();
    assert!(format!("{err:#}").contains("out of range"));
}
