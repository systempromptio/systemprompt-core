//! What the shell tool asks before a command runs, and that nothing runs
//! after a refusal.

use serde_json::json;
use systemprompt_engine::tools::ToolError;

use super::posix::{run, run_input};
use crate::support::{Gate, Project, slash};

#[tokio::test]
async fn each_part_of_a_compound_command_is_asked_about() {
    let project = Project::new();
    let called = run(
        &project,
        "git status && npm run build -- --watch | grep ok; echo $(date)",
    )
    .await;
    let asked = called.gate.asked();
    called.ok();
    let shell = asked
        .iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert_eq!(
        shell.patterns,
        [
            "git status",
            "npm run build -- --watch",
            "grep ok",
            "echo $(…)",
            "date"
        ]
    );
    assert_eq!(
        shell.always,
        [
            "git status *",
            "npm run build *",
            "grep *",
            "echo *",
            "date *"
        ]
    );
}

#[tokio::test]
async fn a_redirect_target_outside_the_project_is_asked_about() {
    let project = Project::new();
    let outside = project.outside();
    let target = slash(&outside.join("out.txt"));
    let called = run(&project, &format!("echo hi > {target}")).await;
    let asked = called.gate.asked();
    called.ok();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[0].patterns, [format!("{}/*", slash(&outside))]);
    assert!(outside.join("out.txt").exists());
}

#[tokio::test]
async fn an_outside_directory_is_asked_once_across_workdir_and_directory_changes() {
    let project = Project::new();
    let outside = project.outside();
    let called = run_input(
        &project,
        json!({"command": format!("cd {} && ls {}", slash(&outside), slash(&outside)), "workdir": slash(&outside)}),
        Gate::default(),
    )
    .await;
    let asked = called.gate.asked();
    called.ok();
    let external: Vec<_> = asked
        .iter()
        .filter(|request| request.permission == "external_directory")
        .collect();
    assert_eq!(external.len(), 1);
    assert_eq!(external[0].patterns.len(), 1, "{:?}", external[0].patterns);
}

#[tokio::test]
async fn a_home_directory_path_is_asked_about_as_home() {
    let project = Project::new();
    let called = run(&project, "ls ~/notes/today.txt").await;
    let asked = called.gate.asked();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[0].patterns, ["~/notes/*"]);
}

#[tokio::test]
async fn nothing_runs_after_an_outside_or_shell_refusal() {
    for refused in ["external_directory", "shell"] {
        let project = Project::new();
        let outside = project.outside();
        let marker = project.root.join("marker");
        let command = format!("touch {} && ls {}", slash(&marker), slash(&outside));
        let called = run_input(
            &project,
            json!({"command": command}),
            Gate::refusing(refused),
        )
        .await;
        assert!(matches!(called.err(), ToolError::Rejected), "{refused}");
        assert!(!marker.exists(), "{refused}");
    }
}

#[tokio::test]
#[ignore = "parity ledger: a directory change is asked about as a command of its own"]
async fn a_directory_change_is_not_asked_about_separately() {
    let project = Project::new();
    std::fs::create_dir_all(project.root.join("src")).unwrap();
    let called = run(&project, "cd src && ls").await;
    let shell = called
        .gate
        .asked()
        .into_iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert_eq!(shell.patterns, ["ls"]);
}

#[tokio::test]
#[ignore = "parity ledger: a line the scanner cannot analyse is split best-effort, not asked verbatim"]
async fn an_unanalysable_command_is_asked_about_verbatim() {
    let project = Project::new();
    let called = run(&project, "echo 'unterminated; rm -rf x").await;
    let shell = called
        .gate
        .asked()
        .into_iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert_eq!(shell.patterns, ["echo 'unterminated; rm -rf x"]);
    assert!(shell.always.is_empty());
}

#[tokio::test]
async fn a_line_with_no_command_is_asked_about_as_written() {
    let project = Project::new();
    let called = run(&project, "A=1").await;
    let shell = called
        .gate
        .asked()
        .into_iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert_eq!(shell.patterns, ["A=1"]);
}

#[tokio::test]
async fn substitutions_and_redirect_targets_are_each_visible_to_the_question() {
    let project = Project::new();
    let called = run(&project, "cat $(ls) | sort > sorted.txt").await;
    let asked = called.gate.asked();
    let shell = asked
        .iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert!(
        shell.patterns.contains(&"ls".to_owned()),
        "{:?}",
        shell.patterns
    );
    assert!(
        shell.patterns.contains(&"sort".to_owned()),
        "{:?}",
        shell.patterns
    );
    assert_eq!(shell.metadata["command"], "cat $(ls) | sort > sorted.txt");
}

#[tokio::test]
async fn a_bare_numeric_symlink_operand_reaches_outside_without_an_outside_question() {
    let project = Project::new();
    let outside = project.outside();
    std::os::unix::fs::symlink(&outside, project.root.join("123")).unwrap();
    let called = run(&project, "cd 123 && pwd -P").await;
    let asked = called.gate.permissions();
    let out = called.ok();
    assert_eq!(asked, ["shell"]);
    assert_eq!(out.output.trim(), slash(&crate::support::dunce(&outside)));
}

#[tokio::test]
async fn a_directory_operand_continued_over_a_line_break_is_asked_about_as_one_path() {
    let project = Project::new();
    let outside = slash(&project.outside());
    let (head, tail) = outside.split_at(outside.len() / 2);
    let called = run(&project, &format!("cd {head}\\\n{tail} && pwd")).await;
    let asked = called.gate.asked();
    let external: Vec<_> = asked
        .iter()
        .filter(|request| request.permission == "external_directory")
        .collect();
    assert_eq!(external.len(), 1, "{asked:?}");
    assert_eq!(external[0].patterns, [format!("{outside}/*")]);
}

#[tokio::test]
async fn a_process_substitution_inside_a_test_is_asked_about() {
    let project = Project::new();
    let called = run(&project, "[[ -n <(printf reached > marker) ]]; wait \"$!\"").await;
    let asked = called.gate.asked();
    let shell = asked
        .iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert!(
        shell
            .patterns
            .iter()
            .any(|pattern| pattern.starts_with("printf")),
        "{:?}",
        shell.patterns
    );
    assert!(
        shell
            .patterns
            .iter()
            .any(|pattern| pattern.starts_with("wait")),
        "{:?}",
        shell.patterns
    );
}

#[tokio::test]
async fn nothing_runs_when_a_process_substitution_inside_a_test_is_refused() {
    let project = Project::new();
    let called = run_input(
        &project,
        json!({"command": "[[ -n <(printf reached > marker) ]]; wait \"$!\""}),
        Gate::refusing("shell"),
    )
    .await;
    assert!(matches!(called.err(), ToolError::Rejected));
    assert!(!project.exists("marker"));
}

#[tokio::test]
async fn arithmetic_is_asked_about_as_one_command() {
    let project = Project::new();
    let sum = run(&project, "echo $((1 + 1))").await;
    let asked = sum.gate.asked();
    let shell = asked
        .iter()
        .find(|request| request.permission == "shell")
        .unwrap();
    assert_eq!(shell.patterns, ["echo $((1 + 1))"]);
    assert_eq!(shell.always, ["echo *"]);
}

#[tokio::test]
async fn arithmetic_and_directory_changes_run_and_ask_as_ordinary_commands() {
    let project = Project::new();
    std::fs::create_dir_all(project.root.join("one").join("two")).unwrap();
    let sum = run(&project, "echo $((1 + 1))").await;
    assert_eq!(sum.ok().output.trim(), "2");
    let nested = run(&project, "cd one&&cd two&&pwd").await;
    let asked = nested.gate.asked();
    assert!(
        asked
            .iter()
            .all(|request| request.permission != "external_directory"),
        "{asked:?}"
    );
    let out = nested.ok().output;
    assert!(out.trim().ends_with("one/two"), "{out}");
}
