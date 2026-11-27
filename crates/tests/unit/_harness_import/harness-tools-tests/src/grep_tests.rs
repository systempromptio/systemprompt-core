use std::fmt::Write as _;
use std::path::Path;

use harness_tools::{GrepTool, SearchLimits, Tool, ToolError, ToolOutput};
use serde_json::{Value, json};

use crate::support::{put, recorded};

async fn grep(root: &Path, input: Value) -> Result<ToolOutput, ToolError> {
    let (ctx, _) = recorded(root);
    GrepTool::new(SearchLimits::default())
        .run(input, &ctx)
        .await
}

fn files(root: &Path) {
    put(root, "a.rs", b"one\ntwo\nneedle here\nfour\n");
    put(root, "b.txt", b"needle too\n");
    put(root, "c.bin", b"needle\x00\x01");
}

#[tokio::test]
async fn finds_matches_with_context_and_include() {
    let dir = tempfile::tempdir().unwrap();
    files(dir.path());
    let out = grep(
        dir.path(),
        json!({"pattern": "needle", "include": "*.rs", "context": 1}),
    )
    .await
    .unwrap();
    assert_eq!(
        out.output,
        "1 match in 1 file\n\na.rs\n  2- two\n  3: needle here\n  4- four\n"
    );
    assert_eq!(out.metadata["matches"], 1);
}

#[tokio::test]
async fn skips_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    files(dir.path());
    let out = grep(dir.path(), json!({"pattern": "needle"}))
        .await
        .unwrap();
    assert_eq!(out.metadata["matches"], 2);
    assert_eq!(out.metadata["files"], 2);
    assert!(!out.output.contains("c.bin"));
}

#[tokio::test]
async fn caps_total_matches() {
    let dir = tempfile::tempdir().unwrap();
    let content: String = (0..300).fold(String::new(), |mut out, i| {
        writeln!(out, "hit {i}").unwrap();
        out
    });
    put(dir.path(), "many.txt", content.as_bytes());
    let out = grep(dir.path(), json!({"pattern": "hit"})).await.unwrap();
    assert_eq!(
        out.metadata["matches"],
        SearchLimits::default().grep_matches
    );
    assert_eq!(out.metadata["truncated"], true);
    assert!(out.output.contains("stopped at the match limit"));
}

#[tokio::test]
async fn invalid_expressions_keep_their_cause() {
    let dir = tempfile::tempdir().unwrap();
    let err = grep(dir.path(), json!({"pattern": "("})).await.unwrap_err();
    assert!(matches!(err, ToolError::Malformed { .. }));
    assert!(
        err.to_string()
            .starts_with("invalid input: invalid regular expression: ")
    );
}
