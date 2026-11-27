use serde_json::json;
use systemprompt_engine::tools::{EditTool, Tool, ToolError, ToolOutput};

use crate::tools::support::{invalid_message, put, recorded};

async fn edit(
    content: &str,
    old: &str,
    new: &str,
    all: bool,
) -> Result<(String, ToolOutput), ToolError> {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "f.txt", content.as_bytes());
    let (ctx, _) = recorded(dir.path());
    let input = json!({"path": "f.txt", "old_string": old, "new_string": new, "replace_all": all});
    let output = EditTool.run(input, &ctx).await?;
    Ok((
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        output,
    ))
}

fn strategy(output: &ToolOutput) -> &str {
    output.metadata["strategy"].as_str().unwrap()
}

#[tokio::test]
async fn exact_unique_match() {
    let (text, out) = edit(
        "let a = 1;\nlet b = 2;\n",
        "let b = 2;",
        "let b = 3;",
        false,
    )
    .await
    .unwrap();
    assert_eq!(text, "let a = 1;\nlet b = 3;\n");
    assert_eq!(strategy(&out), "exact");
    assert_eq!(out.metadata["replacements"], 1);
}

#[tokio::test]
async fn ambiguous_unless_replace_all() {
    let content = "x();\ny();\nx();\n";
    let err = edit(content, "x();", "z();", false).await.unwrap_err();
    assert!(invalid_message(&err).contains("2 places"));
    let (text, out) = edit(content, "x();", "z();", true).await.unwrap();
    assert_eq!(text, "z();\ny();\nz();\n");
    assert_eq!(out.metadata["replacements"], 2);
}

#[tokio::test]
async fn indentation_flexible_keeps_file_indent() {
    let content = "fn a() {\n        let x = 1;\n        let y = 2;\n}\n";
    let old = "    let x = 1;\n    let y = 2;";
    let new = "    let x = 10;\n    let y = 2;";
    let (text, out) = edit(content, old, new, false).await.unwrap();
    assert_eq!(strategy(&out), "indentation-flexible");
    assert_eq!(
        text,
        "fn a() {\n        let x = 10;\n        let y = 2;\n}\n"
    );
    assert!(out.output.contains("re-read before the next edit"));
}

#[tokio::test]
async fn line_trimmed_ignores_per_line_whitespace() {
    let content = "if a {\n\tcall();  \n}\n";
    let (text, out) = edit(
        content,
        "if a {\n  call();\n}",
        "if a {\n\tcall2();\n}",
        false,
    )
    .await
    .unwrap();
    assert_eq!(strategy(&out), "line-trimmed");
    assert_eq!(text, "if a {\n\tcall2();\n}\n");
}

#[tokio::test]
async fn whitespace_normalized_collapses_runs() {
    let content = "let  x =   compute( a,  b );\nnext();\n";
    let (text, out) = edit(
        content,
        "let x = compute( a, b );",
        "let x = compute(a, b);",
        false,
    )
    .await
    .unwrap();
    assert_eq!(strategy(&out), "whitespace-normalized");
    assert_eq!(text, "let x = compute(a, b);\nnext();\n");
}

#[tokio::test]
async fn trailing_newline_in_needle_consumes_line_end() {
    let (text, out) = edit("a\n  b  \nc\n", "b\n", "", false).await.unwrap();
    assert_eq!(strategy(&out), "indentation-flexible");
    assert_eq!(text, "a\nc\n");
}

#[tokio::test]
async fn missing_needles_and_full_files_are_refused() {
    let missing = edit("abc", "zzz", "y", false).await.unwrap_err();
    assert!(invalid_message(&missing).contains("was not found"));
    let blank = edit("abc\n", "\n\n", "y", false).await.unwrap_err();
    assert!(invalid_message(&blank).contains("was not found"));
    let empty = edit("abc", "", "y", false).await.unwrap_err();
    assert!(invalid_message(&empty).contains("already has content"));
    let same = edit("abc", "abc", "abc", false).await.unwrap_err();
    assert!(invalid_message(&same).contains("identical"));
}

#[tokio::test]
async fn stricter_rung_wins_over_looser_ones() {
    let (text, out) = edit("  foo();\nfoo();\n", "foo();\n", "bar();\n", true)
        .await
        .unwrap();
    assert_eq!(strategy(&out), "exact");
    assert_eq!(text, "  bar();\nbar();\n");
}

#[tokio::test]
async fn keeps_crlf_and_bom_and_asks_edit() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "\u{feff}one\r\ntwo\r\n").unwrap();
    let (ctx, gate) = recorded(dir.path());
    let out = EditTool
        .run(
            json!({"path": "a.txt", "old_string": "two", "new_string": "2"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(&file).unwrap(),
        "\u{feff}one\r\n2\r\n".as_bytes()
    );
    assert!(out.metadata["diff"].as_str().unwrap().contains("+2"));
    let asked = gate.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].permission, "edit");
    assert_eq!(asked[0].patterns, ["a.txt"]);
}

#[tokio::test]
async fn lf_files_stay_lf_whatever_the_new_text_uses() {
    let (text, _) = edit("a\nb\n", "b", "x\r\ny", false).await.unwrap();
    assert_eq!(text, "a\nx\ny\n");
}

#[tokio::test]
async fn files_that_are_not_utf8_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "bin.dat", b"\xff\xfe\x00junk");
    let (ctx, _) = recorded(dir.path());
    let err = EditTool
        .run(
            json!({"path": "bin.dat", "old_string": "junk", "new_string": "x"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(invalid_message(&err).contains("not UTF-8"));
}

#[tokio::test]
async fn creates_files_and_rejects_ambiguity() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    EditTool
        .run(
            json!({"path": "new/b.txt", "old_string": "", "new_string": "x\nx\n"}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new/b.txt")).unwrap(),
        "x\nx\n"
    );
    let err = EditTool
        .run(
            json!({"path": "new/b.txt", "old_string": "x", "new_string": "y"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(invalid_message(&err).contains("2 places"));
}

#[tokio::test]
async fn edits_outside_the_worktree_ask_external_directory_first() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("proj");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(dir.path().join("outside.txt"), "a\n").unwrap();
    let (ctx, gate) = recorded(&root);
    EditTool
        .run(
            json!({"path": "../outside.txt", "old_string": "a", "new_string": "b"}),
            &ctx,
        )
        .await
        .unwrap();
    let asked = gate.asked();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[1].permission, "edit");
}
