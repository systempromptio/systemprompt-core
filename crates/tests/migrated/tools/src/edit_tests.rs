//! The edit tool beyond the engine's own matching tests: what a refusal or a
//! miss reveals, typography, mixed line endings and no-op results.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_engine::tools::{EditTool, FileObserver, Tool, ToolError};

use crate::support::{Called, Gate, Project};

async fn edit(project: &Project, old: &str, new: &str) -> Called {
    project
        .call(
            &EditTool,
            json!({"path": "f.txt", "old_string": old, "new_string": new}),
        )
        .await
}

#[tokio::test]
#[ignore = "parity ledger: a refused edit still reports whether old_string matched"]
async fn a_refused_edit_reveals_nothing_about_whether_the_text_matched() {
    let project = Project::new();
    project.put("f.txt", b"alpha\n");
    let hit = project
        .call_with(
            &EditTool,
            json!({"path": "f.txt", "old_string": "alpha", "new_string": "beta"}),
            Gate::refusing("edit"),
        )
        .await
        .message();
    let miss = project
        .call_with(
            &EditTool,
            json!({"path": "f.txt", "old_string": "gamma", "new_string": "beta"}),
            Gate::refusing("edit"),
        )
        .await
        .message();
    assert_eq!(hit, miss);
}

#[tokio::test]
#[ignore = "parity ledger: a miss names neither the closest lines nor an edit already applied"]
async fn a_miss_points_at_the_closest_lines_or_at_the_new_text_already_present() {
    let project = Project::new();
    project.put("f.txt", b"fn main() {\n    let total = 1;\n}\n");
    let near = edit(&project, "let totl = 1;", "let total = 2;")
        .await
        .message();
    assert!(near.contains("let total = 1;"), "{near}");
    let applied = edit(&project, "let total = 0;", "let total = 1;")
        .await
        .message();
    assert!(applied.contains("already"), "{applied}");
}

#[tokio::test]
#[ignore = "parity ledger: typographic quotes and dashes are not normalised when matching"]
async fn typographic_characters_match_only_after_exact_text_fails() {
    let project = Project::new();
    project.put(
        "f.txt",
        "say(\u{201c}hi\u{201d}) \u{2014} done\n".as_bytes(),
    );
    edit(&project, "say(\"hi\") -- done", "said").await.ok();
    assert_eq!(project.text("f.txt"), "said\n");
}

#[tokio::test]
async fn exact_text_with_typographic_characters_is_matched_as_written() {
    let project = Project::new();
    project.put("f.txt", "say(\u{201c}hi\u{201d})\nsay(\"hi\")\n".as_bytes());
    edit(&project, "say(\"hi\")", "plain").await.ok();
    assert_eq!(project.text("f.txt"), "say(\u{201c}hi\u{201d})\nplain\n");
}

#[tokio::test]
#[ignore = "parity ledger: a file with mixed line endings is rewritten with one style"]
async fn mixed_line_endings_are_kept_region_by_region() {
    let project = Project::new();
    project.put("f.txt", b"one\r\ntwo\r\nthree\nfour\n");
    edit(&project, "four", "4").await.ok();
    assert_eq!(project.read("f.txt"), b"one\r\ntwo\r\nthree\n4\n");
}

#[tokio::test]
#[ignore = "parity ledger: an edit whose result equals the file reports success"]
async fn an_edit_that_leaves_the_file_unchanged_is_an_error() {
    let project = Project::new();
    project.put("f.txt", b"value\n");
    let called = edit(&project, "value  ", "value").await;
    assert!(called.result.is_err());
}

#[tokio::test]
async fn identical_old_and_new_text_is_refused() {
    let project = Project::new();
    project.put("f.txt", b"x\n");
    let err = edit(&project, "x", "x").await.message();
    assert!(err.contains("identical"), "{err}");
}

#[derive(Debug)]
struct Upper;

#[async_trait]
impl FileObserver for Upper {
    async fn before_write(&self, _path: &Path) {}

    async fn after_write(&self, path: &Path) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        std::fs::write(path, text.to_uppercase()).ok()?;
        None
    }
}

#[tokio::test]
#[ignore = "parity ledger: the returned diff shows the edit before the formatter ran"]
async fn the_returned_diff_reflects_the_formatted_content() {
    let project = Project::new();
    project.put("f.txt", b"a\n");
    let mut ctx = project.context(Arc::new(Gate::default()));
    ctx.observer = Some(Arc::new(Upper));
    let out = EditTool
        .run(
            json!({"path": "f.txt", "old_string": "a", "new_string": "b"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(out.metadata["diff"].as_str().unwrap().contains("+B"));
}

#[tokio::test]
async fn a_missing_file_is_named_and_creating_it_is_suggested() {
    let project = Project::new();
    let err = edit(&project, "a", "b").await.message();
    assert!(err.contains("file not found: f.txt"), "{err}");
    assert!(err.contains("empty old_string"), "{err}");
}

#[tokio::test]
#[ignore = "parity ledger: editing a directory fails with an I/O error that does not say it is a directory"]
async fn editing_a_directory_says_it_is_a_directory() {
    let project = Project::new();
    project.put("f.txt/inner", b"");
    let err = edit(&project, "a", "b").await.message();
    assert!(err.contains("directory"), "{err}");
}

#[tokio::test]
async fn an_absolute_path_inside_the_project_is_edited_without_an_outside_ask() {
    let project = Project::new();
    let path = project.put("f.txt", b"a\n");
    let called = project
        .call(
            &EditTool,
            json!({"path": path.to_string_lossy(), "old_string": "a", "new_string": "b"}),
        )
        .await;
    assert_eq!(called.gate.permissions(), ["edit"]);
    called.ok();
    assert_eq!(project.text("f.txt"), "b\n");
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "parity ledger: editing through an in-project symlink to an outside file asks outside approval"]
async fn editing_through_an_in_project_symlink_needs_only_edit_permission() {
    let project = Project::new();
    let target = crate::support::put(&project.outside(), "t.txt", b"a\n");
    std::os::unix::fs::symlink(&target, project.root.join("f.txt")).unwrap();
    let called = edit(&project, "a", "b").await;
    assert_eq!(called.gate.permissions(), ["edit"]);
}

#[tokio::test]
async fn nothing_is_written_when_either_question_is_refused() {
    for refused in ["external_directory", "edit"] {
        let project = Project::new();
        let file = crate::support::put(&project.outside(), "o.txt", b"a\n");
        let called = project
            .call_with(
                &EditTool,
                json!({"path": file.to_string_lossy(), "old_string": "a", "new_string": "b"}),
                Gate::refusing(refused),
            )
            .await;
        assert!(matches!(called.err(), ToolError::Rejected), "{refused}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "a\n");
    }
}
