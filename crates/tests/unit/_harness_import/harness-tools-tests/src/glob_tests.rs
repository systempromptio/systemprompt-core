use std::path::Path;

use harness_tools::{GlobTool, SearchLimits, Tool, ToolError};
use serde_json::json;

use crate::support::{put, recorded};

fn tree(root: &Path) {
    put(root, ".gitignore", b"target/\n");
    put(root, "src/a.rs", b"");
    put(root, "src/nested/b.rs", b"");
    put(root, "target/c.rs", b"");
}

async fn glob(root: &Path, pattern: &str) -> Vec<String> {
    let (ctx, _) = recorded(root);
    let out = GlobTool::new(SearchLimits::default())
        .run(json!({"pattern": pattern}), &ctx)
        .await
        .unwrap();
    let mut names: Vec<String> = out.output.lines().map(str::to_owned).collect();
    names.sort();
    names
}

#[tokio::test]
async fn bare_patterns_match_at_any_depth_and_respect_ignore_files() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    assert_eq!(
        glob(dir.path(), "*.rs").await,
        ["src/a.rs", "src/nested/b.rs"]
    );
}

#[tokio::test]
async fn relative_and_rooted_patterns_are_normalised() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    assert_eq!(glob(dir.path(), "./src/*.rs").await, ["src/a.rs"]);
    let rooted = format!(
        "{}/src/**/*.rs",
        dir.path().to_string_lossy().replace('\\', "/")
    );
    assert_eq!(
        glob(dir.path(), &rooted).await,
        ["src/a.rs", "src/nested/b.rs"]
    );
}

#[tokio::test]
async fn nothing_found_says_so() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    assert_eq!(glob(dir.path(), "*.py").await, ["No files found."]);
}

#[tokio::test]
async fn invalid_patterns_keep_their_cause() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let err = GlobTool::new(SearchLimits::default())
        .run(json!({"pattern": "a[b"}), &ctx)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Malformed { .. }));
    assert!(
        err.to_string()
            .starts_with("invalid input: invalid glob pattern: ")
    );
}
