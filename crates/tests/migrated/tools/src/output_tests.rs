//! Capping what a tool returns: the preview, the saved full text, and edge
//! cases of line and byte limits.

use systemprompt_engine::tools::{Keep, OutputLimits, truncate_output};

fn limits(max_lines: usize, max_bytes: usize) -> OutputLimits {
    OutputLimits {
        max_lines,
        max_bytes,
    }
}

#[cfg(unix)]
#[tokio::test]
async fn the_preview_survives_when_the_full_text_cannot_be_saved() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o555)).unwrap();
    let probe = std::fs::write(data.join("probe"), b"");
    let out = truncate_output(
        "a\nb\nc\n".to_owned(),
        limits(1, 1000),
        Keep::Head,
        &data,
        "x",
    )
    .await;
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();
    if probe.is_ok() {
        // skip-ok: running as a user that bypasses directory modes (root) cannot make
        // the save fail
        return;
    }
    assert!(out.truncated);
    assert_eq!(out.spill, None);
    assert!(
        out.text
            .starts_with("a\n\n[Output truncated: 2 more lines not shown.")
    );
    assert!(out.text.contains("could not be saved"));
}

#[tokio::test]
#[ignore = "parity ledger: the byte cut can split a flag or a joined emoji sequence"]
async fn the_cut_never_splits_a_flag_or_a_joined_emoji() {
    let dir = tempfile::tempdir().unwrap();
    let flags = "\u{1f1fa}\u{1f1f8}".repeat(10);
    let out = truncate_output(flags, limits(10, 12), Keep::Head, dir.path(), "flags").await;
    let kept = out.text.split("\n\n[").next().unwrap();
    assert_eq!(kept.chars().count() % 2, 0, "{kept:?}");
}

#[tokio::test]
async fn the_cut_never_splits_a_character() {
    let dir = tempfile::tempdir().unwrap();
    let text = "\u{1f600}".repeat(10);
    let head = truncate_output(text.clone(), limits(10, 10), Keep::Head, dir.path(), "h").await;
    assert!(
        head.text
            .starts_with("\u{1f600}\u{1f600}\n\n[Output truncated")
    );
    let tail = truncate_output(text, limits(10, 10), Keep::Tail, dir.path(), "t").await;
    assert!(tail.text.ends_with("]\n\n\u{1f600}\u{1f600}"));
}

#[tokio::test]
async fn a_zero_line_limit_shows_no_lines() {
    let dir = tempfile::tempdir().unwrap();
    let out = truncate_output(
        "a\nb\n".to_owned(),
        limits(0, 1000),
        Keep::Head,
        dir.path(),
        "z",
    )
    .await;
    assert!(out.truncated);
    assert!(
        out.text
            .starts_with("\n\n[Output truncated: 2 more lines not shown."),
        "{:?}",
        out.text
    );
}

#[tokio::test]
async fn a_trailing_newline_is_not_another_line() {
    let dir = tempfile::tempdir().unwrap();
    let out = truncate_output(
        "a\nb\n".to_owned(),
        limits(2, 1000),
        Keep::Head,
        dir.path(),
        "n",
    )
    .await;
    assert!(!out.truncated);
    assert_eq!(out.text, "a\nb\n");
}

#[tokio::test]
#[ignore = "parity ledger: a line cut by the byte limit is counted as shown"]
async fn a_line_cut_by_the_byte_limit_ends_before_its_line_break() {
    let dir = tempfile::tempdir().unwrap();
    let out = truncate_output(
        "abcdefgh\nij\n".to_owned(),
        limits(10, 4),
        Keep::Head,
        dir.path(),
        "c",
    )
    .await;
    assert!(
        out.text
            .starts_with("abcd\n\n[Output truncated: 2 more lines"),
        "{:?}",
        out.text
    );
}

#[tokio::test]
#[ignore = "parity ledger: a blank last line of the preview is trimmed away"]
async fn a_blank_last_line_of_the_preview_is_counted() {
    let dir = tempfile::tempdir().unwrap();
    let out = truncate_output(
        "a\n\nb\nc\n".to_owned(),
        limits(2, 1000),
        Keep::Head,
        dir.path(),
        "b",
    )
    .await;
    assert!(
        out.text
            .starts_with("a\n\n\n[Output truncated: 2 more lines"),
        "{:?}",
        out.text
    );
}

#[tokio::test]
async fn text_that_fits_is_returned_unchanged_and_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let text = "fits\n".to_owned();
    let out = truncate_output(
        text.clone(),
        OutputLimits::default(),
        Keep::Tail,
        dir.path(),
        "f",
    )
    .await;
    assert_eq!((out.text, out.truncated, out.spill), (text, false, None));
    assert!(!dir.path().join("tool-output").exists());
}

#[tokio::test]
async fn the_saved_file_name_is_made_safe_and_the_preview_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let out = truncate_output(
        "a\nb\n".to_owned(),
        limits(1, 1000),
        Keep::Head,
        dir.path(),
        "../x y",
    )
    .await;
    let spill = out.spill.unwrap();
    assert_eq!(spill.file_name().unwrap(), "___x_y.txt");
    assert!(spill.starts_with(dir.path().join("tool-output")));
    assert!(out.text.contains("(4 bytes, 2 lines)"));
}
