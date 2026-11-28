//! The patch tool: reading the envelope leniently, locating hunks, moves,
//! failures and the permission questions it asks.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_engine::tools::{FileObserver, PatchTool, Tool, ToolError, ToolOutput};

use crate::support::{Called, Gate, Project, slash};

const BOM: &str = "\u{feff}";

fn envelope(body: &str) -> String {
    format!("%% begin patch\n{body}%% end patch\n")
}

async fn apply(project: &Project, text: &str) -> Called {
    project.call(&PatchTool, json!({"patch": text})).await
}

async fn update(body: &str, hunks: &str) -> Result<String, ToolError> {
    let project = Project::new();
    project.put("f.txt", body.as_bytes());
    let called = apply(&project, &envelope(&format!("%% update f.txt\n{hunks}"))).await;
    called.result.map(|_| project.text("f.txt"))
}

fn files(out: &ToolOutput) -> Vec<serde_json::Value> {
    out.metadata["files"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn add_update_and_delete_sections_apply_in_order() {
    let project = Project::new();
    project.put("keep.txt", b"one\ntwo\n");
    project.put("old.txt", b"bye\n");
    let text = envelope(
        "%% add new.txt\n+hello\n+world\n%% update keep.txt\n one\n-two\n+2\n%% delete old.txt\n",
    );
    let out = apply(&project, &text).await.ok();
    assert_eq!(project.text("new.txt"), "hello\nworld\n");
    assert_eq!(project.text("keep.txt"), "one\n2\n");
    assert!(!project.exists("old.txt"));
    assert!(out.output.contains("A new.txt\nM keep.txt\nD old.txt"));
}

#[tokio::test]
async fn an_envelope_with_no_sections_is_refused() {
    let project = Project::new();
    let err = apply(&project, "%% begin patch\n%% end patch\n")
        .await
        .message();
    assert!(err.contains("no file changes"), "{err}");
}

#[tokio::test]
async fn a_final_empty_added_line_is_kept() {
    let project = Project::new();
    apply(&project, &envelope("%% add a.txt\n+first\n+\n"))
        .await
        .ok();
    assert_eq!(project.text("a.txt"), "first\n\n");
}

#[tokio::test]
async fn a_preamble_before_the_envelope_is_ignored() {
    let project = Project::new();
    let text = format!(
        "Here is the change you asked for.\nworkdir: /tmp\n{}",
        envelope("%% add a.txt\n+x\n")
    );
    apply(&project, &text).await.ok();
    assert_eq!(project.text("a.txt"), "x\n");
}

#[tokio::test]
async fn a_missing_opening_marker_is_named() {
    let project = Project::new();
    let err = apply(&project, "%% add a.txt\n+x\n%% end patch\n")
        .await
        .message();
    assert!(
        err.contains("must start with the line `%% begin patch`"),
        "{err}"
    );
}

#[tokio::test]
async fn whitespace_after_the_closing_marker_is_allowed() {
    let project = Project::new();
    let text = "%% begin patch\n%% add a.txt\n+x\n%% end patch   \n";
    apply(&project, text).await.ok();
    assert_eq!(project.text("a.txt"), "x\n");
}

#[tokio::test]
async fn a_here_document_wrapper_is_stripped_in_each_spelling() {
    for (open, close) in [
        ("cat <<'EOF'", "EOF"),
        ("<<EOF", "EOF"),
        ("cat <<\"PATCH\"", "PATCH"),
    ] {
        let project = Project::new();
        let text = format!("{open}\n{}{close}\n", envelope("%% add a.txt\n+x\n"));
        apply(&project, &text).await.ok();
        assert_eq!(project.text("a.txt"), "x\n", "{open}");
    }
}

#[tokio::test]
#[ignore = "parity ledger: a here-document wrapper whose delimiters disagree is accepted"]
async fn a_malformed_here_document_wrapper_is_refused() {
    let project = Project::new();
    let text = format!("cat <<'EOF'\n{}END\n", envelope("%% add a.txt\n+x\n"));
    assert!(apply(&project, &text).await.result.is_err());
}

#[tokio::test]
async fn a_hunk_header_padded_with_whitespace_is_read() {
    let done = update(
        "a\nfn main() {\n    x();\n}\n",
        "@@   fn main() {   @@\n-    x();\n+    y();\n",
    )
    .await
    .unwrap();
    assert_eq!(done, "a\nfn main() {\n    y();\n}\n");
}

#[tokio::test]
#[ignore = "parity ledger: section markers indented with whitespace are not recognised"]
async fn whitespace_around_section_markers_is_tolerated() {
    let project = Project::new();
    let text = "  %% begin patch  \n  %% add a.txt  \n+x\n  %% end patch  \n";
    apply(&project, text).await.ok();
    assert_eq!(project.text("a.txt"), "x\n");
}

#[tokio::test]
async fn whitespace_inside_section_directives_is_tolerated() {
    let project = Project::new();
    let text = "%% begin patch\n%%   add    a.txt\n+x\n%%    end    patch\n";
    apply(&project, text).await.ok();
    assert_eq!(project.text("a.txt"), "x\n");
}

#[tokio::test]
#[ignore = "parity ledger: the opening marker must be spelled with single spaces"]
async fn whitespace_inside_the_opening_marker_is_tolerated() {
    let project = Project::new();
    let text = "%%   begin patch\n%%   add    a.txt\n+x\n%%    end    patch\n";
    apply(&project, text).await.ok();
    assert_eq!(project.text("a.txt"), "x\n");
}

#[tokio::test]
async fn crlf_patch_lines_lose_one_carriage_return_only() {
    let project = Project::new();
    let text = "%% begin patch\r\n%% add a.txt\r\n+plain\r\n+kept\r\r\n%% end patch\r\n";
    apply(&project, text).await.ok();
    assert_eq!(project.text("a.txt"), "plain\nkept\r\n");
}

#[tokio::test]
async fn the_end_of_file_marker_anchors_its_hunk_and_may_precede_another() {
    let body = "x\ny\nx\ny\n";
    let done = update(body, "@@\n-x\n+first\n%% end of file\n").await.err();
    assert!(done.is_some(), "x is not among the last lines of the file");
    let anchored = update(body, "@@\n x\n-y\n+last\n%% end of file\n")
        .await
        .unwrap();
    assert_eq!(anchored, "x\ny\nx\nlast\n");
    let then_more = update("a\nb\nc\n", "@@\n-c\n+C\n%% end of file\n@@\n-a\n+A\n")
        .await
        .unwrap();
    assert_eq!(then_more, "A\nb\nC\n");
}

#[tokio::test]
async fn an_update_without_a_first_hunk_header_is_read() {
    let done = update("one\ntwo\n", " one\n-two\n+three\n").await.unwrap();
    assert_eq!(done, "one\nthree\n");
}

#[tokio::test]
async fn indented_marker_lines_inside_a_hunk_are_context() {
    let body = "%% update x\n%% rename-to y\n%% end of file\nlast\n";
    let done = update(
        body,
        "@@\n %% update x\n %% rename-to y\n %% end of file\n-last\n+final\n",
    )
    .await
    .unwrap();
    assert_eq!(done, "%% update x\n%% rename-to y\n%% end of file\nfinal\n");
}

#[tokio::test]
async fn context_keeps_the_files_indentation_and_bare_empty_lines_are_context() {
    let body = "fn a() {\n\n        call();\n}\n";
    let done = update(body, "@@\n fn a() {\n\n-    call();\n+    other();\n }\n")
        .await
        .unwrap();
    assert_eq!(done, "fn a() {\n\n    other();\n}\n");
}

#[tokio::test]
async fn malformed_sections_are_refused_with_the_line_named() {
    let cases = [
        (
            "%% add a.txt\nno plus\n",
            "every line of an added file starts with +",
        ),
        ("%% delete a.txt\nstray\n", "a deleted file takes no lines"),
        ("%% update a.txt\n", "has no hunks"),
        ("%% update a.txt\n@@ anchor\n", "changes nothing"),
        (
            "%% update a.txt\n@@\n*bad\n",
            "hunk lines start with a space",
        ),
        ("%% update\n", "needs a file path"),
        ("%% frobnicate a.txt\n", "unknown directive"),
    ];
    for (body, expected) in cases {
        let project = Project::new();
        project.put("a.txt", b"x\n");
        let err = apply(&project, &envelope(body)).await.message();
        assert!(err.contains(expected), "{body:?}: {err}");
        assert_eq!(project.text("a.txt"), "x\n");
    }
}

#[tokio::test]
async fn a_whitespace_tolerant_match_keeps_the_byte_order_mark() {
    let project = Project::new();
    project.put(
        "f.txt",
        format!("{BOM}fn a() {{\n\tcall( 1 );\n}}\n").as_bytes(),
    );
    apply(
        &project,
        &envelope("%% update f.txt\n@@\n-  call(  1  );\n+\tcall(2);\n"),
    )
    .await
    .ok();
    assert_eq!(
        project.text("f.txt"),
        format!("{BOM}fn a() {{\n\tcall(2);\n}}\n")
    );
}

#[tokio::test]
async fn several_hunks_including_pure_additions_and_deletions_apply_in_order() {
    let body = "a\nb\nc\nd\ne\n";
    let done = update(body, "@@\n-a\n+A\n@@ b\n+b2\n@@\n-d\n")
        .await
        .unwrap();
    assert_eq!(done, "A\nb\nb2\nc\ne\n");
}

#[tokio::test]
async fn an_empty_file_can_be_updated_and_ends_with_a_newline() {
    let done = update("", "@@\n+first\n+second\n").await.unwrap();
    assert_eq!(done, "first\nsecond\n");
}

#[tokio::test]
#[ignore = "parity ledger: typographic quotes, dashes and minus signs are not normalised when matching"]
async fn punctuation_variants_match_when_exact_text_does_not() {
    let done = update(
        "say(\u{201c}hi\u{201d});\nx = a \u{2212} b;\n",
        "@@\n-say(\"hi\");\n-x = a - b;\n+done();\n",
    )
    .await
    .unwrap();
    assert_eq!(done, "done();\n");
}

#[tokio::test]
async fn unusual_spaces_match_as_spaces() {
    let done = update("let\u{a0}x = 1;\n", "@@\n-let x = 1;\n+let x = 2;\n")
        .await
        .unwrap();
    assert_eq!(done, "let x = 2;\n");
}

#[tokio::test]
async fn ellipses_are_not_normalised_into_dots() {
    let err = update("wait\u{2026}\n", "@@\n-wait...\n+go\n").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn a_later_exact_match_wins_over_an_earlier_tolerant_one() {
    let done = update("  target \nother\ntarget\n", "@@\n-target\n+hit\n")
        .await
        .unwrap();
    assert_eq!(done, "  target \nother\nhit\n");
}

#[tokio::test]
async fn an_end_anchored_hunk_never_falls_back_to_a_match_elsewhere() {
    let err = update("tail\nmiddle\nend\n", "@@\n-tail\n+x\n%% end of file\n").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn a_hunk_that_is_not_found_shows_the_lines_it_expected() {
    let err = update("a\n\nb\n", "@@\n a\n-b\n+c\n").await.unwrap_err();
    let text = err.to_string();
    assert!(text.contains("hunk 1 were not found"), "{text}");
    assert!(text.contains("  a\n  b"), "{text}");
}

#[tokio::test]
async fn a_file_can_be_replaced_by_a_directory_of_the_same_name() {
    let project = Project::new();
    project.put("node", b"was a file\n");
    apply(
        &project,
        &envelope("%% delete node\n%% add node/inner.txt\n+x\n"),
    )
    .await
    .ok();
    assert_eq!(project.text("node/inner.txt"), "x\n");
}

#[tokio::test]
async fn deleted_line_counts_do_not_depend_on_a_trailing_newline() {
    for body in [&b"a\nb\n"[..], &b"a\nb"[..]] {
        let project = Project::new();
        project.put("f.txt", body);
        let out = apply(&project, &envelope("%% delete f.txt\n")).await.ok();
        assert_eq!(files(&out)[0]["deletions"], 2, "{body:?}");
    }
}

#[derive(Debug)]
struct Formatter;

#[async_trait]
impl FileObserver for Formatter {
    async fn before_write(&self, _path: &Path) {}

    async fn after_write(&self, path: &Path) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        std::fs::write(path, text.to_uppercase()).ok()?;
        None
    }
}

#[tokio::test]
#[ignore = "parity ledger: the returned diff shows the patch before the formatter ran"]
async fn the_returned_diff_shows_the_formatted_content() {
    let project = Project::new();
    project.put("f.txt", b"a\n");
    let mut ctx = project.context(Arc::new(Gate::default()));
    ctx.observer = Some(Arc::new(Formatter));
    let out = PatchTool
        .run(
            json!({"patch": envelope("%% update f.txt\n-a\n+b\n")}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(out.metadata["diff"].as_str().unwrap().contains("+B"));
}

#[tokio::test]
async fn a_file_can_be_moved_and_changed_and_the_move_is_in_the_metadata() {
    let project = Project::new();
    project.put("old.txt", b"one\ntwo\n");
    let out = apply(
        &project,
        &envelope("%% update old.txt\n%% rename-to sub/new.txt\n one\n-two\n+2\n"),
    )
    .await
    .ok();
    assert!(!project.exists("old.txt"));
    assert_eq!(project.text("sub/new.txt"), "one\n2\n");
    assert_eq!(files(&out)[0]["from"], "old.txt");
    assert_eq!(files(&out)[0]["path"], "sub/new.txt");
    assert!(out.output.contains("R old.txt -> sub/new.txt"));
}

#[tokio::test]
async fn a_file_can_be_moved_without_changing_it() {
    let project = Project::new();
    project.put("a.txt", b"same\n");
    apply(&project, &envelope("%% update a.txt\n%% rename-to b.txt\n"))
        .await
        .ok();
    assert_eq!(project.text("b.txt"), "same\n");
    assert!(!project.exists("a.txt"));
}

#[tokio::test]
#[ignore = "parity ledger: moving onto an existing file is refused"]
async fn a_move_onto_an_existing_file_replaces_it() {
    let project = Project::new();
    project.put("a.txt", b"new\n");
    project.put("b.txt", b"old\n");
    apply(&project, &envelope("%% update a.txt\n%% rename-to b.txt\n"))
        .await
        .ok();
    assert_eq!(project.text("b.txt"), "new\n");
}

#[tokio::test]
#[ignore = "parity ledger: a move that changes only the case of the name is refused on case-insensitive file systems"]
async fn a_move_that_changes_only_case_renames_the_file() {
    let project = Project::new();
    project.put("readme.txt", b"x\n");
    apply(
        &project,
        &envelope("%% update readme.txt\n%% rename-to README.txt\n"),
    )
    .await
    .ok();
    let names: Vec<String> = std::fs::read_dir(&project.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"README.txt".to_owned()), "{names:?}");
    assert!(!names.contains(&"readme.txt".to_owned()), "{names:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn moving_a_symlink_leaves_its_target_untouched() {
    let project = Project::new();
    project.put("target.txt", b"kept\n");
    std::os::unix::fs::symlink(
        project.root.join("target.txt"),
        project.root.join("link.txt"),
    )
    .unwrap();
    apply(
        &project,
        &envelope("%% update link.txt\n%% rename-to moved.txt\n"),
    )
    .await
    .ok();
    assert_eq!(project.text("target.txt"), "kept\n");
    assert!(std::fs::symlink_metadata(project.root.join("link.txt")).is_err());
}

#[tokio::test]
async fn a_move_target_resolves_against_the_working_directory() {
    let project = Project::new();
    project.put("pkg/a.txt", b"x\n");
    let mut ctx = project.context(Arc::new(Gate::default()));
    ctx.cwd = project.root.join("pkg");
    let out = PatchTool
        .run(
            json!({"patch": envelope("%% update a.txt\n%% rename-to b.txt\n")}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(project.text("pkg/b.txt"), "x\n");
    assert_eq!(files(&out)[0]["path"], "pkg/b.txt");
}

#[tokio::test]
async fn a_file_that_is_not_utf8_is_refused_and_nothing_applies() {
    let project = Project::new();
    project.put("bin.dat", b"\xff\xfe\x00\x01");
    let err = apply(
        &project,
        &envelope("%% add first.txt\n+x\n%% update bin.dat\n-a\n+b\n"),
    )
    .await
    .message();
    assert!(err.contains("not UTF-8"), "{err}");
    assert!(!project.exists("first.txt"));
}

#[tokio::test]
async fn deleting_or_updating_a_directory_is_refused() {
    let project = Project::new();
    project.put("dir/a.txt", b"x\n");
    for body in ["%% delete dir\n", "%% update dir\n-x\n+y\n"] {
        let called = apply(&project, &envelope(body)).await;
        assert!(called.result.is_err(), "{body}");
    }
    assert!(project.exists("dir/a.txt"));
}

#[tokio::test]
async fn a_missing_target_of_update_or_delete_is_named() {
    let project = Project::new();
    let update = apply(&project, &envelope("%% update gone.txt\n-a\n+b\n"))
        .await
        .message();
    assert!(
        update.contains("cannot update gone.txt: file not found"),
        "{update}"
    );
    let delete = apply(&project, &envelope("%% delete gone.txt\n"))
        .await
        .message();
    assert!(
        delete.contains("cannot delete gone.txt: file not found"),
        "{delete}"
    );
}

#[tokio::test]
async fn a_second_hunk_whose_context_is_missing_fails_the_patch() {
    let err = update("a\nb\nc\n", "@@\n-a\n+A\n@@\n-zzz\n+y\n")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("hunk 2 were not found"), "{err}");
}

#[tokio::test]
async fn the_patch_text_is_required() {
    let project = Project::new();
    let err = project.call(&PatchTool, json!({})).await.message();
    assert!(err.contains("`patch` is required"), "{err}");
}

#[tokio::test]
#[ignore = "parity ledger: a move onto the same path is refused as a duplicate"]
async fn a_move_onto_its_own_path_keeps_the_file() {
    let project = Project::new();
    project.put("a.txt", b"one\ntwo\n");
    apply(
        &project,
        &envelope("%% update a.txt\n%% rename-to a.txt\n-one\n+1\n"),
    )
    .await
    .ok();
    assert_eq!(project.text("a.txt"), "1\ntwo\n");
}

#[tokio::test]
async fn the_diff_of_a_file_with_a_byte_order_mark_does_not_invent_a_first_line_change() {
    let project = Project::new();
    project.put("f.txt", format!("{BOM}one\ntwo\n").as_bytes());
    let out = apply(&project, &envelope("%% update f.txt\n one\n-two\n+2\n"))
        .await
        .ok();
    let diff = out.metadata["diff"].as_str().unwrap().to_owned();
    assert!(!diff.contains("-one") && !diff.contains("+one"), "{diff}");
    assert_eq!(files(&out)[0]["additions"], 1);
    assert_eq!(files(&out)[0]["deletions"], 1);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "parity ledger: a write that fails part-way does not report the changes already made"]
async fn a_failed_write_reports_what_already_landed_and_which_path_failed() {
    use std::os::unix::fs::PermissionsExt as _;
    let project = Project::new();
    let locked = project.put("locked.txt", b"x\n");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
    let probe = std::fs::OpenOptions::new().write(true).open(&locked);
    let called = apply(
        &project,
        &envelope("%% add first.txt\n+x\n%% update locked.txt\n-x\n+y\n"),
    )
    .await;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    if probe.is_ok() {
        // skip-ok: running as a user that bypasses file modes (root) cannot make the
        // write fail
        return;
    }
    assert!(project.exists("first.txt"));
    let err = called.message();
    assert!(err.contains("first.txt"), "{err}");
    assert!(err.contains("locked.txt"), "{err}");
}

#[tokio::test]
#[ignore = "parity ledger: an added file whose path exists is refused instead of overwritten"]
async fn an_added_file_overwrites_an_existing_one() {
    let project = Project::new();
    project.put("a.txt", b"old\n");
    apply(&project, &envelope("%% add a.txt\n+new\n"))
        .await
        .ok();
    assert_eq!(project.text("a.txt"), "new\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parity ledger: concurrent patches of one file are not serialised and can lose a change"]
async fn concurrent_patches_of_one_file_are_serialised() {
    let project = Project::new();
    let body: String = (0..16).map(|index| format!("line{index}\n")).collect();
    project.put("f.txt", body.as_bytes());
    let ctx = project.context(Arc::new(Gate::default()));
    let mut handles = Vec::new();
    for index in 0..16 {
        let ctx = ctx.clone();
        handles.push(tokio::spawn(async move {
            let text = envelope(&format!("%% update f.txt\n-line{index}\n+done{index}\n"));
            PatchTool.run(json!({"patch": text}), &ctx).await
        }));
    }
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
    assert!(!project.text("f.txt").contains("line"));
}

#[tokio::test]
#[ignore = "parity ledger: files are read before outside approval is asked"]
async fn outside_approval_is_asked_before_an_outside_file_is_read() {
    let project = Project::new();
    let outside = project.outside();
    let path = slash(&outside.join("missing.txt"));
    let called = project
        .call_with(
            &PatchTool,
            json!({"patch": envelope(&format!("%% update {path}\n-a\n+b\n"))}),
            Gate::refusing("external_directory"),
        )
        .await;
    assert_eq!(called.gate.permissions(), ["external_directory"]);
    assert!(matches!(called.err(), ToolError::Rejected));
}

#[tokio::test]
async fn an_outside_file_asks_outside_approval_then_edit() {
    let project = Project::new();
    let outside = project.outside();
    let file = crate::support::put(&outside, "o.txt", b"a\n");
    let called = apply(
        &project,
        &envelope(&format!("%% update {}\n-a\n+b\n", slash(&file))),
    )
    .await;
    let asked = called.gate.asked();
    called.ok();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[0].always, [format!("{}/*", slash(&outside))]);
    assert_eq!(asked[1].permission, "edit");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "b\n");
}

#[tokio::test]
async fn a_refusal_of_either_question_leaves_every_file_untouched() {
    for refused in ["external_directory", "edit"] {
        let project = Project::new();
        let file = crate::support::put(&project.outside(), "o.txt", b"a\n");
        project.put("in.txt", b"a\n");
        let text = envelope(&format!(
            "%% update in.txt\n-a\n+b\n%% update {}\n-a\n+b\n",
            slash(&file)
        ));
        let called = project
            .call_with(&PatchTool, json!({"patch": text}), Gate::refusing(refused))
            .await;
        assert!(matches!(called.err(), ToolError::Rejected), "{refused}");
        assert_eq!(project.text("in.txt"), "a\n");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "a\n");
    }
}

#[tokio::test]
async fn a_sibling_path_inside_the_project_is_internal() {
    let project = Project::new();
    project.put("b/target.txt", b"a\n");
    let mut ctx = project.context(Arc::new(Gate::default()));
    ctx.cwd = project.root.join("a");
    std::fs::create_dir_all(&ctx.cwd).unwrap();
    let gate = Arc::new(Gate::default());
    ctx.gate = gate.clone();
    PatchTool
        .run(
            json!({"patch": envelope("%% update ../b/target.txt\n-a\n+b\n")}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(gate.permissions(), ["edit"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "parity ledger: an in-project symlink to an outside file asks outside approval"]
async fn an_in_project_symlink_to_an_outside_file_needs_no_outside_approval() {
    let project = Project::new();
    let target = crate::support::put(&project.outside(), "t.txt", b"a\n");
    std::os::unix::fs::symlink(&target, project.root.join("link.txt")).unwrap();
    let called = apply(&project, &envelope("%% update link.txt\n-a\n+b\n")).await;
    assert_eq!(called.gate.permissions(), ["edit"]);
}

#[tokio::test]
async fn a_move_destination_outside_is_asked_about_at_its_directory() {
    let project = Project::new();
    let outside = project.outside();
    project.put("a.txt", b"x\n");
    let called = apply(
        &project,
        &envelope(&format!(
            "%% update a.txt\n%% rename-to {}\n",
            slash(&outside.join("b.txt"))
        )),
    )
    .await;
    let asked = called.gate.asked();
    called.ok();
    assert_eq!(asked[0].permission, "external_directory");
    assert_eq!(asked[0].patterns, [format!("{}/*", slash(&outside))]);
    assert!(outside.join("b.txt").exists());
}

#[tokio::test]
async fn each_outside_file_is_asked_about_even_under_one_parent() {
    let project = Project::new();
    let outside = project.outside();
    let text = envelope(&format!(
        "%% add {}\n+1\n%% add {}\n+2\n",
        slash(&outside.join("one.txt")),
        slash(&outside.join("two.txt"))
    ));
    let called = apply(&project, &text).await;
    let asked = called.gate.permissions();
    called.ok();
    assert_eq!(asked, ["external_directory", "external_directory", "edit"]);
}
