use systemprompt_engine::workspace::{
    DiffSummary, FileStatus, LineStats, file_diff, line_stats, unified_diff,
};

#[test]
fn counts_added_and_removed_lines() {
    let stats = line_stats("a\nb\nc\n", "a\nB\nc\nd\n");
    assert_eq!(
        stats,
        LineStats {
            additions: 2,
            deletions: 1
        }
    );
}

#[test]
fn patch_has_headers_and_hunks() {
    let patch = unified_diff("src/x.txt", "one\ntwo\n", "one\nthree\n");
    assert!(patch.starts_with("--- a/src/x.txt\n+++ b/src/x.txt\n"));
    assert!(patch.contains("-two\n"));
    assert!(patch.contains("+three\n"));
}

#[test]
fn equal_text_has_no_patch() {
    assert_eq!(unified_diff("x", "same\n", "same\n"), "");
}

#[test]
fn added_file_diffs_against_dev_null() {
    let diff = file_diff("new.txt", None, Some(b"a\nb\n".as_slice()));
    assert_eq!(diff.status, FileStatus::Added);
    assert_eq!((diff.additions, diff.deletions), (2, 0));
    assert!(diff.patch.starts_with("--- /dev/null\n+++ b/new.txt\n"));
}

#[test]
fn deleted_file_counts_deletions() {
    let diff = file_diff("old.txt", Some(b"a\nb\nc\n".as_slice()), None);
    assert_eq!(diff.status, FileStatus::Deleted);
    assert_eq!((diff.additions, diff.deletions), (0, 3));
}

#[test]
fn crlf_lines_are_kept_in_the_patch() {
    let diff = file_diff(
        "w.txt",
        Some(b"a\r\nb\r\n".as_slice()),
        Some(b"a\r\nc\r\n".as_slice()),
    );
    assert_eq!((diff.additions, diff.deletions), (1, 1));
    assert!(diff.patch.contains("+c\r\n"));
}

#[test]
fn binary_content_is_not_patched() {
    let diff = file_diff(
        "b.bin",
        Some([0u8, 1, 2].as_slice()),
        Some([0u8, 1, 3].as_slice()),
    );
    assert!(diff.binary);
    assert_eq!((diff.additions, diff.deletions), (0, 0));
    assert!(diff.patch.starts_with("Binary files"));
}

#[test]
fn summary_adds_up() {
    let diffs = [
        file_diff("a", None, Some(b"1\n2\n".as_slice())),
        file_diff("b", Some(b"1\n".as_slice()), None),
    ];
    let summary = DiffSummary::from_diffs(&diffs);
    assert_eq!(
        summary,
        DiffSummary {
            additions: 2,
            deletions: 1,
            files: 2
        }
    );
}
