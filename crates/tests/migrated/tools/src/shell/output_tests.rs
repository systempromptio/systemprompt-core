//! Working directory, captured output and how the result reads.

use std::fmt::Write as _;

use serde_json::json;
use systemprompt_engine::tools::{BackgroundJobs, OutputLimits, ShellConfig, Tool};

use super::posix::{run, run_input, tool};
use crate::support::{Gate, Project, slash};

fn lines(count: usize) -> String {
    (1..=count).fold(String::new(), |mut out, index| {
        write!(out, "echo line{index}; ").unwrap();
        out
    })
}

#[tokio::test]
async fn a_relative_workdir_resolves_against_the_project() {
    let project = Project::new();
    std::fs::create_dir_all(project.root.join("sub/dir")).unwrap();
    let out = run_input(
        &project,
        json!({"command": "pwd", "workdir": "sub/dir"}),
        Gate::default(),
    )
    .await
    .ok();
    let shown = crate::support::dunce(&project.root.join("sub/dir"));
    assert_eq!(out.output.trim(), slash(&shown));
    assert_eq!(
        out.metadata["workdir"],
        slash(&project.root.join("sub/dir"))
    );
}

#[tokio::test]
async fn stderr_alone_and_mixed_output_are_captured() {
    let project = Project::new();
    let only = run(&project, "echo oops 1>&2").await.ok();
    assert_eq!(only.output, "oops\n");
    let mixed = run(
        &project,
        "echo out; sleep 0.1; echo err 1>&2; sleep 0.1; echo out2",
    )
    .await
    .ok();
    assert_eq!(mixed.output, "out\nerr\nout2\n");
}

#[tokio::test]
async fn the_exit_status_follows_the_truncation_note() {
    let project = Project::new();
    let shell = tool(
        ShellConfig::default(),
        OutputLimits {
            max_lines: 5,
            max_bytes: 10_000,
        },
        &BackgroundJobs::new(),
    );
    let out = project
        .call(&shell, json!({"command": format!("{} exit 4", lines(50))}))
        .await
        .ok();
    assert!(
        out.output.starts_with("[Output truncated: the first"),
        "{}",
        out.output
    );
    assert!(out.output.ends_with("[Exit code: 4]"), "{}", out.output);
    assert_eq!(out.metadata["truncated"], true);
    assert_eq!(out.metadata["exit_code"], 4);
}

#[tokio::test]
async fn the_truncated_view_counts_whole_lines_and_names_where_the_rest_is() {
    let project = Project::new();
    let shell = tool(
        ShellConfig::default(),
        OutputLimits {
            max_lines: 3,
            max_bytes: 10_000,
        },
        &BackgroundJobs::new(),
    );
    let out = project
        .call(&shell, json!({"command": lines(10)}))
        .await
        .ok();
    assert!(
        out.output
            .starts_with("[Output truncated: the first 7 lines are not shown."),
        "{}",
        out.output
    );
    assert!(
        out.output.ends_with("line8\nline9\nline10\n"),
        "{}",
        out.output
    );
    let saved = out.metadata["output_path"].as_str().unwrap().to_owned();
    assert!(out.output.contains(&saved));
    assert!(std::fs::read_to_string(saved).unwrap().contains("line1\n"));
}

#[tokio::test]
async fn a_single_long_line_starts_its_view_on_a_whole_character() {
    let project = Project::new();
    let shell = tool(
        ShellConfig::default(),
        OutputLimits {
            max_lines: 10,
            max_bytes: 7,
        },
        &BackgroundJobs::new(),
    );
    let out = project
        .call(&shell, json!({"command": "printf 'ééééééééé'"}))
        .await
        .ok();
    assert!(out.output.ends_with("]\n\nééé"), "{}", out.output);
}

#[tokio::test]
async fn the_partial_first_line_is_dropped_from_the_truncated_view() {
    let project = Project::new();
    let shell = tool(
        ShellConfig::default(),
        OutputLimits {
            max_lines: 100,
            max_bytes: 12,
        },
        &BackgroundJobs::new(),
    );
    let out = project
        .call(
            &shell,
            json!({"command": "printf 'aaaaaaaaaa\\nbbbb\\ncccc\\n'"}),
        )
        .await
        .ok();
    assert!(out.output.ends_with("]\n\nbbbb\ncccc\n"), "{}", out.output);
}

#[tokio::test]
#[ignore = "parity ledger: the note does not say how much of a single long line is shown"]
async fn the_note_says_how_much_of_a_single_long_line_is_shown() {
    let project = Project::new();
    let shell = tool(
        ShellConfig::default(),
        OutputLimits {
            max_lines: 10,
            max_bytes: 8,
        },
        &BackgroundJobs::new(),
    );
    let out = project
        .call(&shell, json!({"command": "printf '%040d' 0"}))
        .await
        .ok();
    assert!(out.output.contains("8 of 40 bytes"), "{}", out.output);
}

#[tokio::test]
async fn a_command_with_no_output_says_so_once() {
    let project = Project::new();
    let out = run(&project, "exit 2").await.ok();
    assert_eq!(out.output, "(no output)\n\n[Exit code: 2]");
    assert_eq!(out.output.matches("no output").count(), 1);
}

#[tokio::test]
async fn the_status_is_worded_as_an_exit_code_and_a_signal_has_none() {
    let project = Project::new();
    let failed = run(&project, "echo x; exit 7").await.ok();
    assert!(failed.output.ends_with("[Exit code: 7]"));
    let killed = run(&project, "kill -9 $$").await.ok();
    assert_eq!(killed.metadata["exit_code"], serde_json::Value::Null);
    assert!(
        killed.output.contains("without an exit code"),
        "{}",
        killed.output
    );
}

#[tokio::test]
async fn the_result_names_the_shell_that_ran_it() {
    let project = Project::new();
    let out = run(&project, "true").await.ok();
    assert_eq!(out.metadata["shell"], "a POSIX shell");
}

#[tokio::test]
async fn a_workdir_that_is_a_file_is_refused() {
    let project = Project::new();
    project.put("file.txt", b"");
    let err = run_input(
        &project,
        json!({"command": "true", "workdir": "file.txt"}),
        Gate::default(),
    )
    .await
    .message();
    assert!(err.contains("is not a directory"), "{err}");
}

#[tokio::test]
async fn a_command_runs_in_the_requested_directory_with_pagers_disabled() {
    let project = Project::new();
    let out = run(&project, "echo \"$PAGER $GIT_PAGER $GIT_TERMINAL_PROMPT\"")
        .await
        .ok();
    assert_eq!(out.output, "cat cat 0\n");
}

#[tokio::test]
async fn progress_streams_while_the_command_runs() {
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Seen(Mutex<Vec<serde_json::Value>>);
    impl systemprompt_engine::tools::Progress for Seen {
        fn update(&self, _title: &str, metadata: serde_json::Value) {
            self.0.lock().unwrap().push(metadata);
        }
    }
    let project = Project::new();
    let seen = Arc::new(Seen::default());
    let mut ctx = project.context(Arc::new(Gate::default()));
    ctx.progress = Some(seen.clone());
    super::posix::plain()
        .run(json!({"command": "echo one; sleep 0.3; echo two"}), &ctx)
        .await
        .unwrap();
    assert!(!seen.0.lock().unwrap().is_empty());
}
