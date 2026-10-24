//! Output capture: what the diff marks added or modified is kept within the
//! file and byte bounds, listed from the saved manifest, and served only
//! from inside `outputs/`.

use systemprompt_agent_runs::RunLayout;
use systemprompt_agent_runs::execute::outputs::{MAX_OUTPUT_FILES, capture, list, read};
use systemprompt_agent_runs::execute::{DiffEntry, DiffKind, DiffSummary};

fn diff(entries: &[(&str, DiffKind)]) -> DiffSummary {
    DiffSummary {
        entries: entries
            .iter()
            .map(|(path, kind)| DiffEntry {
                path: (*path).to_owned(),
                kind: *kind,
            })
            .collect(),
        ..DiffSummary::default()
    }
}

#[tokio::test]
async fn capture_keeps_added_and_modified_and_lists_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    layout.create().await.expect("layout");
    std::fs::create_dir_all(layout.workspace.join("out")).expect("dir");
    std::fs::write(layout.workspace.join("out/a.txt"), "alpha").expect("a");
    std::fs::write(layout.workspace.join("b.txt"), "beta").expect("b");
    let summary = diff(&[
        ("out/a.txt", DiffKind::Added),
        ("b.txt", DiffKind::Modified),
        ("gone.txt", DiffKind::Deleted),
        ("../escape.txt", DiffKind::Added),
    ]);
    let kept = capture(&layout.workspace, &summary, &layout)
        .await
        .expect("capture");
    assert_eq!(kept.kept.len(), 2);
    assert_eq!(kept.skipped, ["../escape.txt"]);
    let listed = list(&layout).await.expect("list");
    assert_eq!(listed, kept.kept);
    assert_eq!(
        read(&layout, "out/a.txt").await.expect("read"),
        Some(b"alpha".to_vec())
    );
    assert_eq!(
        read(&layout, "../logs/stdout.jsonl").await.expect("read"),
        None
    );
    assert_eq!(read(&layout, "/etc/passwd").await.expect("read"), None);
}

#[tokio::test]
async fn capture_is_bounded_in_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    layout.create().await.expect("layout");
    let names: Vec<String> = (0..=MAX_OUTPUT_FILES)
        .map(|i| format!("f{i}.txt"))
        .collect();
    for name in &names {
        std::fs::write(layout.workspace.join(name), "x").expect("write");
    }
    let entries: Vec<(&str, DiffKind)> = names
        .iter()
        .map(|n| (n.as_str(), DiffKind::Added))
        .collect();
    let kept = capture(&layout.workspace, &diff(&entries), &layout)
        .await
        .expect("capture");
    assert_eq!(kept.kept.len(), MAX_OUTPUT_FILES);
    assert_eq!(kept.skipped.len(), 1);
}

#[tokio::test]
async fn a_run_without_outputs_lists_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        list(&RunLayout::new(dir.path()))
            .await
            .expect("list")
            .is_empty()
    );
}
