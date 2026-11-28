//! Glob and grep: ignore files, version-control metadata, limits, odd input,
//! search roots and outside approval.

use serde_json::{Value, json};
use systemprompt_engine::tools::{GlobTool, GrepTool, SearchLimits};

use crate::support::{Called, Gate, Project, slash};

fn limits() -> SearchLimits {
    SearchLimits {
        glob_results: 3,
        grep_matches: 3,
        grep_max_file_bytes: 1024 * 1024,
        grep_line_chars: 12,
    }
}

async fn glob(project: &Project, input: Value) -> Called {
    project.call(&GlobTool::new(limits()), input).await
}

async fn grep(project: &Project, input: Value) -> Called {
    project.call(&GrepTool::new(limits()), input).await
}

fn listed(output: &str) -> Vec<String> {
    output
        .lines()
        .take_while(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(unix)]
#[tokio::test]
async fn a_malformed_ignore_pattern_and_a_broken_link_do_not_stop_the_search() {
    let project = Project::new();
    project.put(".gitignore", b"[unclosed\n");
    project.put("src/found.rs", b"needle\n");
    std::os::unix::fs::symlink(project.root.join("absent"), project.root.join("broken")).unwrap();
    let files = glob(&project, json!({"pattern": "*.rs"})).await.ok();
    assert_eq!(listed(&files.output), ["src/found.rs"]);
    let hits = grep(&project, json!({"pattern": "needle"})).await.ok();
    assert!(hits.output.contains("src/found.rs"), "{}", hits.output);
}

#[tokio::test]
#[ignore = "parity ledger: hidden files are always listed; there is no hidden-files choice"]
async fn hidden_files_are_left_out_before_the_result_limit_applies() {
    let project = Project::new();
    for name in [".a.txt", ".b.txt", ".c.txt", "visible.txt"] {
        project.put(name, b"");
    }
    let out = glob(&project, json!({"pattern": "*.txt"})).await.ok();
    assert_eq!(listed(&out.output), ["visible.txt"]);
}

#[tokio::test]
async fn lines_that_are_not_valid_text_are_still_searched() {
    let project = Project::new();
    project.put("latin.txt", b"caf\xe9 needle\n");
    let out = grep(&project, json!({"pattern": "needle"})).await.ok();
    assert_eq!(out.metadata["matches"], 1);
    assert!(out.output.contains("1: caf\u{fffd} need"), "{}", out.output);
}

#[tokio::test]
async fn ignored_files_and_version_control_metadata_are_never_returned() {
    let project = Project::new();
    project.put(".gitignore", b"target/\n*.log\n");
    project.put(".git/config", b"needle\n");
    project.put(".git/HEAD", b"needle\n");
    project.put("target/out.txt", b"needle\n");
    project.put("debug.log", b"needle\n");
    project.put("kept.txt", b"needle\n");
    let files = glob(&project, json!({"pattern": "**/*"})).await.ok();
    let mut names = listed(&files.output);
    names.sort();
    assert_eq!(names, [".gitignore", "kept.txt"]);
    let hits = grep(&project, json!({"pattern": "needle"})).await.ok();
    assert_eq!(hits.metadata["files"], 1);
    assert!(hits.output.contains("kept.txt"));
}

#[tokio::test]
async fn an_over_long_matching_line_is_cut_to_a_preview_on_a_whole_character() {
    let project = Project::new();
    project.put("long.txt", "needle éééééééééééééééé\n".as_bytes());
    let out = grep(&project, json!({"pattern": "needle"})).await.ok();
    assert!(out.output.contains("1: needle ééééé…\n"), "{}", out.output);
}

#[tokio::test]
async fn results_are_bounded_when_no_limit_is_given() {
    let project = Project::new();
    for index in 0..5 {
        project.put(&format!("f{index}.txt"), b"needle\n");
    }
    let files = glob(&project, json!({"pattern": "*.txt"})).await.ok();
    assert_eq!(listed(&files.output).len(), 3);
    assert_eq!(files.metadata["count"], 5);
    assert_eq!(files.metadata["truncated"], true);
    let hits = grep(&project, json!({"pattern": "needle"})).await.ok();
    assert_eq!(hits.metadata["matches"], 3);
    assert_eq!(hits.metadata["truncated"], true);
}

#[tokio::test]
#[ignore = "parity ledger: an empty grep pattern matches every line instead of being refused"]
async fn an_empty_grep_pattern_is_refused() {
    let project = Project::new();
    project.put("a.txt", b"x\n");
    assert!(grep(&project, json!({"pattern": ""})).await.result.is_err());
}

#[tokio::test]
async fn grep_accepts_an_explicit_file_or_directory() {
    let project = Project::new();
    project.put("dir/a.txt", b"needle\n");
    project.put("dir/b.txt", b"needle\n");
    project.put("other.txt", b"needle\n");
    let one = grep(&project, json!({"pattern": "needle", "path": "dir/a.txt"}))
        .await
        .ok();
    assert_eq!(one.metadata["files"], 1);
    assert!(one.output.contains("dir/a.txt"));
    let dir = grep(&project, json!({"pattern": "needle", "path": "dir"}))
        .await
        .ok();
    assert_eq!(dir.metadata["files"], 2);
    assert!(!dir.output.contains("other.txt"));
}

#[tokio::test]
async fn no_matches_is_said_plainly() {
    let project = Project::new();
    project.put("a.txt", b"x\n");
    let out = grep(&project, json!({"pattern": "absent"})).await.ok();
    assert_eq!(out.output, "No matches found.");
}

#[tokio::test]
async fn a_missing_search_path_is_reported_by_both_tools() {
    let project = Project::new();
    let missing = slash(&project.root.join("gone"));
    let globbed = glob(&project, json!({"pattern": "*", "path": "gone"}))
        .await
        .message();
    assert!(
        globbed.contains(&format!("{missing} is not a directory")),
        "{globbed}"
    );
    let grepped = grep(&project, json!({"pattern": "x", "path": "gone"}))
        .await
        .message();
    assert!(
        grepped.contains(&format!("{missing} does not exist")),
        "{grepped}"
    );
}

#[tokio::test]
async fn a_file_given_as_the_glob_root_is_reported() {
    let project = Project::new();
    project.put("a.txt", b"");
    let err = glob(&project, json!({"pattern": "*", "path": "a.txt"}))
        .await
        .message();
    assert!(err.contains("a.txt is not a directory"), "{err}");
}

#[tokio::test]
async fn searching_an_explicit_outside_path_asks_outside_approval() {
    let project = Project::new();
    let outside = project.outside();
    crate::support::put(&outside, "o.txt", b"needle\n");
    let path = outside.to_string_lossy().into_owned();
    let grepped = grep(&project, json!({"pattern": "needle", "path": path.clone()})).await;
    assert_eq!(grepped.gate.permissions(), ["external_directory", "grep"]);
    assert_eq!(grepped.ok().metadata["matches"], 1);
    let file = outside.join("o.txt").to_string_lossy().into_owned();
    let one = grep(&project, json!({"pattern": "needle", "path": file})).await;
    assert_eq!(one.gate.permissions(), ["external_directory", "grep"]);
    let globbed = glob(&project, json!({"pattern": "*.txt", "path": path})).await;
    assert_eq!(globbed.gate.permissions(), ["external_directory", "glob"]);
    let refused = project
        .call_with(
            &GlobTool::new(limits()),
            json!({"pattern": "*.txt", "path": outside.to_string_lossy()}),
            Gate::refusing("external_directory"),
        )
        .await;
    assert!(refused.result.is_err());
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "parity ledger: a symlinked directory inside the project is not followed by glob"]
async fn glob_follows_an_in_project_symlink_to_outside_without_asking() {
    let project = Project::new();
    let outside = project.outside();
    crate::support::put(&outside, "far.txt", b"");
    std::os::unix::fs::symlink(&outside, project.root.join("linked")).unwrap();
    let called = glob(&project, json!({"pattern": "linked/*.txt"})).await;
    assert_eq!(called.gate.permissions(), ["glob"]);
    assert_eq!(listed(&called.ok().output), ["linked/far.txt"]);
}

#[tokio::test]
async fn glob_patterns_match_recursively_with_braces_dotfiles_and_literals() {
    let project = Project::new();
    for name in ["a.rs", "src/b.ts", "src/deep/c.tsx", ".env", "lit[1].txt"] {
        project.put(name, b"");
    }
    let tool = GlobTool::new(SearchLimits::default());
    let run = |pattern: &'static str| {
        let project = &project;
        let tool = &tool;
        async move {
            let mut names = listed(
                &project
                    .call(tool, json!({"pattern": pattern}))
                    .await
                    .ok()
                    .output,
            );
            names.sort();
            names
        }
    };
    assert_eq!(run("**/*.{ts,tsx}").await, ["src/b.ts", "src/deep/c.tsx"]);
    assert_eq!(run("src/*.ts").await, ["src/b.ts"]);
    assert_eq!(run(".env").await, [".env"]);
    assert_eq!(run("*.rs").await, ["a.rs"]);
}

#[tokio::test]
#[ignore = "parity ledger: backslash escapes in glob patterns are turned into path separators"]
async fn an_escaped_glob_character_matches_literally() {
    let project = Project::new();
    project.put("lit[1].txt", b"");
    let out = glob(&project, json!({"pattern": "lit\\[1\\].txt"}))
        .await
        .ok();
    assert_eq!(listed(&out.output), ["lit[1].txt"]);
}
