use std::path::Path;

use serde_json::json;
use systemprompt_engine::tools::{PatchTool, Tool, ToolError, ToolOutput};

use crate::tools::support::{put, recorded};

const SAMPLE: &str = "\
%% begin patch
%% add docs/new.md
+# Title
+
+body
%% update src/lib.rs
%% rename-to src/core.rs
@@ fn main()
     let a = 1;
-    let b = 2;
+    let b = 3;
%% delete old.txt
%% end patch
";

async fn patch(root: &Path, text: &str) -> Result<ToolOutput, ToolError> {
    let (ctx, _) = recorded(root);
    PatchTool.run(json!({ "patch": text }), &ctx).await
}

async fn update(body: &str, hunks: &str) -> Result<String, ToolError> {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "f.rs", body.as_bytes());
    let text = format!("%% begin patch\n%% update f.rs\n{hunks}%% end patch\n");
    patch(dir.path(), &text).await?;
    Ok(std::fs::read_to_string(dir.path().join("f.rs")).unwrap())
}

fn malformed_text(error: &ToolError) -> String {
    assert!(
        matches!(error, ToolError::Malformed { .. }),
        "unexpected {error:?}"
    );
    error.to_string()
}

#[tokio::test]
async fn applies_a_patch_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(
        root,
        "src/lib.rs",
        b"fn main() {\r\n    let a = 1;\r\n    let b = 2;\r\n}\r\n",
    );
    put(root, "old.txt", b"bye\n");
    let (ctx, gate) = recorded(root);
    let out = PatchTool
        .run(json!({ "patch": SAMPLE }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("docs/new.md")).unwrap(),
        "# Title\n\nbody\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/core.rs")).unwrap(),
        "fn main() {\r\n    let a = 1;\r\n    let b = 3;\r\n}\r\n"
    );
    assert!(!root.join("src/lib.rs").exists());
    assert!(!root.join("old.txt").exists());
    assert!(out.output.contains("A docs/new.md"));
    assert!(out.output.contains("R src/lib.rs -> src/core.rs"));
    assert!(out.output.contains("D old.txt"));
    assert_eq!(out.title, "3 files");
    let asked = gate.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].permission, "edit");
}

#[tokio::test]
async fn nothing_is_written_when_a_hunk_fails() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.txt", b"one\n");
    let text =
        "%% begin patch\n%% add b.txt\n+new\n%% update a.txt\n@@\n-missing\n+x\n%% end patch";
    let err = patch(dir.path(), text).await.unwrap_err();
    assert!(malformed_text(&err).contains("the lines of hunk 1 were not found"));
    assert!(!dir.path().join("b.txt").exists());
}

#[tokio::test]
async fn rejects_malformed_patches() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("no envelope", "must start with the line"),
        ("%% begin patch\n%% end patch\n", "contains no file changes"),
        (
            "%% begin patch\n%% add a\nnot plus\n",
            "patch line 3: every line of an added file",
        ),
        (
            "%% begin patch\n%% update a\n@@\n same\n",
            "changes nothing",
        ),
        (
            "%% begin patch\n%% frobnicate a\n",
            "patch line 2: unknown directive",
        ),
        (
            "%% begin patch\n%% delete a\nline\n",
            "a deleted file takes no lines",
        ),
    ];
    for (text, expected) in cases {
        let err = patch(dir.path(), text).await.unwrap_err();
        assert!(
            malformed_text(&err).contains(expected),
            "{text:?} gave {err}"
        );
    }
}

#[tokio::test]
async fn applies_hunks_in_order_using_anchors() {
    let body = "fn a() {\n    x();\n}\n\nfn b() {\n    x();\n}\n";
    let done = update(body, "@@ fn b() {\n-    x();\n+    y();\n")
        .await
        .unwrap();
    assert_eq!(done, "fn a() {\n    x();\n}\n\nfn b() {\n    y();\n}\n");
}

#[tokio::test]
async fn tolerates_whitespace_drift_and_keeps_file_context() {
    let body = "start\n\tkeep_me();   \n\tdrop_me();\nend\n";
    let done = update(body, "@@\n  keep_me();\n-  drop_me();\n+\tadded();\n")
        .await
        .unwrap();
    assert_eq!(done, "start\n\tkeep_me();   \n\tadded();\nend\n");
}

#[tokio::test]
async fn pure_additions_go_after_anchor_or_at_end() {
    let after_anchor = update("a\nb\nc", "@@ a\n+inserted\n").await.unwrap();
    assert_eq!(after_anchor, "a\ninserted\nb\nc");
    let at_end = update("a\nb\nc", "@@\n+tail\n").await.unwrap();
    assert_eq!(at_end, "a\nb\nc\ntail");
}

#[tokio::test]
async fn end_of_file_hunks_match_the_last_lines() {
    let done = update("x\ny\nx\ny\n", "@@\n x\n-y\n+z\n%% end of file\n")
        .await
        .unwrap();
    assert_eq!(done, "x\ny\nx\nz\n");
}

#[tokio::test]
async fn missing_context_is_reported() {
    let missing = update("a\nb\n", "@@\n-absent\n+present\n")
        .await
        .unwrap_err();
    assert!(malformed_text(&missing).contains("f.rs: the lines of hunk 1 were not found"));
    let anchored = update("a\n", "@@ nowhere\n-a\n+b\n").await.unwrap_err();
    assert!(malformed_text(&anchored).contains("the anchor"));
}

#[tokio::test]
async fn a_path_may_appear_only_once() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.txt", b"one\n");
    let text = "%% begin patch\n%% update a.txt\n@@\n-one\n+two\n%% delete a.txt\n%% end patch";
    let err = patch(dir.path(), text).await.unwrap_err();
    assert!(
        matches!(err, ToolError::InvalidInput { message } if message.contains("more than once"))
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_rename_whose_source_cannot_be_removed_names_the_written_destination() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "src/a.txt", b"x\n");
    let src = dir.path().join("src");
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o555)).unwrap();
    let probe = std::fs::File::create(src.join("probe"));
    let result = patch(
        dir.path(),
        "%% begin patch\n%% update src/a.txt\n%% rename-to b.txt\n-x\n+y\n%% end patch\n",
    )
    .await;
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o755)).unwrap();
    if probe.is_ok() {
        // skip-ok: running as a user that bypasses file modes (root) cannot make the
        // removal fail
        return;
    }
    let message = result.unwrap_err().to_string();
    assert!(message.contains("already changed: b.txt"), "{message}");
    assert!(message.contains("part-way at src/a.txt"), "{message}");
}
