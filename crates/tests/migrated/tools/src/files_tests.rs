//! Whole-file writes: creating, overwriting, byte-order marks, outside paths,
//! denials and concurrent writers.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_engine::tools::{EditTool, FileObserver, Tool, ToolError, WriteTool};

use crate::support::{Gate, Project, slash};

const BOM: &str = "\u{feff}";

#[derive(Debug, Default)]
struct Formatter;

#[async_trait]
impl FileObserver for Formatter {
    async fn before_write(&self, _path: &Path) {}

    async fn after_write(&self, path: &Path) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        std::fs::write(path, text.replace("  ", " ")).ok()?;
        Some("formatted".to_owned())
    }
}

#[tokio::test]
async fn writing_a_relative_new_file_creates_it_once_and_asks_edit_once() {
    let project = Project::new();
    let called = project
        .call(
            &WriteTool,
            json!({"path": "src/new.rs", "content": "fn main() {}\n"}),
        )
        .await;
    let asked = called.gate.permissions();
    let out = called.ok();
    assert_eq!(asked, ["edit"]);
    assert_eq!(out.metadata["created"], true);
    assert!(out.output.starts_with("Created src/new.rs (1 lines)"));
    assert_eq!(project.text("src/new.rs"), "fn main() {}\n");
}

#[tokio::test]
async fn a_new_file_creates_its_missing_parent_directories() {
    let project = Project::new();
    project
        .call(&WriteTool, json!({"path": "a/b/c/d.txt", "content": "x"}))
        .await
        .ok();
    assert_eq!(project.text("a/b/c/d.txt"), "x");
}

#[tokio::test]
async fn the_formatter_runs_on_the_written_file_and_its_note_is_reported() {
    let project = Project::new();
    let gate = Arc::new(Gate::default());
    let mut ctx = project.context(gate);
    ctx.observer = Some(Arc::new(Formatter));
    let out = WriteTool
        .run(json!({"path": "f.txt", "content": "a  b"}), &ctx)
        .await
        .unwrap();
    assert_eq!(project.text("f.txt"), "a b");
    assert!(out.output.ends_with("formatted"));
    assert_eq!(out.metadata["diagnostics"], "formatted");
}

#[tokio::test]
async fn a_new_file_without_a_formatter_reports_no_diagnostics() {
    let project = Project::new();
    let out = project
        .call(&WriteTool, json!({"path": "plain.txt", "content": "x\n"}))
        .await
        .ok();
    assert_eq!(out.metadata["diagnostics"], serde_json::Value::Null);
    assert_eq!(out.output, "Created plain.txt (1 lines).");
}

#[tokio::test]
async fn overwriting_an_existing_file_says_it_was_overwritten() {
    let project = Project::new();
    project.put("f.txt", b"one\ntwo\n");
    let out = project
        .call(
            &WriteTool,
            json!({"path": "f.txt", "content": "one\nthree\n"}),
        )
        .await
        .ok();
    assert_eq!(out.metadata["created"], false);
    assert_eq!(out.output, "Overwrote f.txt (2 lines, +1 -1).");
    assert_eq!(project.text("f.txt"), "one\nthree\n");
}

#[tokio::test]
async fn writing_the_same_content_again_gives_the_same_stable_result() {
    let project = Project::new();
    project.put("f.txt", b"same\n");
    let out = project
        .call(&WriteTool, json!({"path": "f.txt", "content": "same\n"}))
        .await
        .ok();
    assert_eq!(out.output, "Overwrote f.txt (1 lines, +0 -0).");
    assert_eq!(project.text("f.txt"), "same\n");
}

#[tokio::test]
#[ignore = "parity ledger: overwriting a file drops its byte-order mark"]
async fn overwriting_keeps_exactly_one_byte_order_mark() {
    let project = Project::new();
    project.put("f.txt", format!("{BOM}old\n").as_bytes());
    project
        .call(&WriteTool, json!({"path": "f.txt", "content": "new\n"}))
        .await
        .ok();
    assert_eq!(project.text("f.txt"), format!("{BOM}new\n"));
    project
        .call(
            &WriteTool,
            json!({"path": "f.txt", "content": format!("{BOM}newer\n")}),
        )
        .await
        .ok();
    assert_eq!(project.text("f.txt"), format!("{BOM}newer\n"));
}

#[tokio::test]
async fn an_absolute_path_inside_the_project_is_written_without_an_outside_ask() {
    let project = Project::new();
    let path = project.root.join("abs.txt");
    let called = project
        .call(
            &WriteTool,
            json!({"path": path.to_string_lossy(), "content": "x"}),
        )
        .await;
    assert_eq!(called.gate.permissions(), ["edit"]);
    called.ok();
    assert_eq!(project.text("abs.txt"), "x");
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "parity ledger: writing through an in-project symlink to an outside file asks outside approval"]
async fn writing_through_an_in_project_symlink_needs_only_edit_permission() {
    let project = Project::new();
    let outside = project.outside();
    let target = crate::support::put(&outside, "target.txt", b"old");
    std::os::unix::fs::symlink(&target, project.root.join("link.txt")).unwrap();
    let called = project
        .call(&WriteTool, json!({"path": "link.txt", "content": "new"}))
        .await;
    assert_eq!(called.gate.permissions(), ["edit"]);
}

#[tokio::test]
async fn an_outside_absolute_path_asks_outside_approval_before_edit_and_is_written() {
    let project = Project::new();
    let outside = project.outside();
    let path = outside.join("out.txt");
    let called = project
        .call(
            &WriteTool,
            json!({"path": path.to_string_lossy(), "content": "x"}),
        )
        .await;
    let asked = called.gate.asked();
    called.ok();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[0].always, [format!("{}/*", slash(&outside))]);
    assert_eq!(asked[1].permission, "edit");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "x");
}

#[tokio::test]
async fn nothing_is_written_when_outside_approval_is_refused() {
    let project = Project::new();
    let path = project.outside().join("out.txt");
    let called = project
        .call_with(
            &WriteTool,
            json!({"path": path.to_string_lossy(), "content": "x"}),
            Gate::refusing("external_directory"),
        )
        .await;
    assert_eq!(called.gate.permissions(), ["external_directory"]);
    assert!(matches!(called.err(), ToolError::Rejected));
    assert!(!path.exists());
}

#[tokio::test]
async fn nothing_is_written_when_edit_permission_is_refused() {
    let project = Project::new();
    project.put("f.txt", b"old");
    let called = project
        .call_with(
            &WriteTool,
            json!({"path": "f.txt", "content": "new"}),
            Gate::refusing("edit"),
        )
        .await;
    assert!(matches!(called.err(), ToolError::Rejected));
    assert_eq!(project.text("f.txt"), "old");
    let fresh = project
        .call_with(
            &WriteTool,
            json!({"path": "fresh.txt", "content": "new"}),
            Gate::refusing("edit"),
        )
        .await;
    assert!(fresh.result.is_err());
    assert!(!project.exists("fresh.txt"));
}

#[tokio::test]
async fn created_text_has_its_line_endings_normalised() {
    let project = Project::new();
    project
        .call(
            &EditTool,
            json!({"path": "new.txt", "old_string": "", "new_string": "a\r\nb\r\n"}),
        )
        .await
        .ok();
    assert_eq!(project.text("new.txt"), "a\nb\n");
}

#[tokio::test]
async fn writes_to_different_files_proceed_independently() {
    let project = Project::new();
    let ctx = project.context(Arc::new(Gate::default()));
    let writes = (0..16).map(|index| {
        let ctx = ctx.clone();
        async move {
            WriteTool
                .run(
                    json!({"path": format!("f{index}.txt"), "content": format!("{index}")}),
                    &ctx,
                )
                .await
        }
    });
    let results = futures_join(writes).await;
    assert!(results.iter().all(Result::is_ok));
    for index in 0..16 {
        assert_eq!(project.text(&format!("f{index}.txt")), format!("{index}"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parity ledger: concurrent edits of one file are not serialised and can lose an edit"]
async fn concurrent_edits_of_one_file_are_serialised() {
    let project = Project::new();
    let body: String = (0..32).map(|index| format!("token{index}\n")).collect();
    project.put("shared.txt", body.as_bytes());
    let ctx = project.context(Arc::new(Gate::default()));
    let edits = (0..32).map(|index| {
        let ctx = ctx.clone();
        async move {
            EditTool
                .run(
                    json!({
                        "path": "shared.txt",
                        "old_string": format!("token{index}\n"),
                        "new_string": format!("done{index}\n"),
                    }),
                    &ctx,
                )
                .await
        }
    });
    let results = futures_join(edits).await;
    assert!(results.iter().all(Result::is_ok));
    let text = project.text("shared.txt");
    assert!(!text.contains("token"), "{text}");
}

#[tokio::test]
#[ignore = "parity ledger: repeated leading byte-order marks are kept, one more is added"]
async fn an_edit_leaves_exactly_one_leading_byte_order_mark() {
    let project = Project::new();
    project.put("f.txt", format!("{BOM}{BOM}alpha\n").as_bytes());
    project
        .call(
            &EditTool,
            json!({"path": "f.txt", "old_string": "alpha", "new_string": "beta"}),
        )
        .await
        .ok();
    assert_eq!(project.text("f.txt"), format!("{BOM}beta\n"));
}

#[tokio::test]
async fn an_edit_adds_no_mark_to_a_file_without_one_and_leaves_inner_marks_alone() {
    let project = Project::new();
    project.put("f.txt", format!("a{BOM}b\nalpha\n").as_bytes());
    project
        .call(
            &EditTool,
            json!({"path": "f.txt", "old_string": "alpha", "new_string": "beta"}),
        )
        .await
        .ok();
    assert_eq!(project.text("f.txt"), format!("a{BOM}b\nbeta\n"));
}

#[tokio::test]
async fn an_edit_keeps_a_single_leading_mark() {
    let project = Project::new();
    project.put("f.txt", format!("{BOM}alpha\n").as_bytes());
    project
        .call(
            &EditTool,
            json!({"path": "f.txt", "old_string": "alpha", "new_string": "beta"}),
        )
        .await
        .ok();
    assert_eq!(project.text("f.txt"), format!("{BOM}beta\n"));
}

async fn futures_join<F, T>(futures: impl Iterator<Item = F>) -> Vec<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let handles: Vec<_> = futures.map(tokio::spawn).collect();
    let mut out = Vec::with_capacity(handles.len());
    for handle in handles {
        out.push(handle.await.unwrap());
    }
    out
}
