use std::fmt::Write as _;

use systemprompt_engine::tools::{Keep, OutputLimits, truncate_output};

fn numbered(n: usize) -> String {
    (1..=n).fold(String::new(), |mut out, i| {
        writeln!(out, "line {i}").unwrap();
        out
    })
}

#[tokio::test]
async fn short_text_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let result = truncate_output(
        "a\nb\n".to_owned(),
        OutputLimits::default(),
        Keep::Head,
        dir.path(),
        "x",
    )
    .await;
    assert!(!result.truncated);
    assert_eq!(result.text, "a\nb\n");
    assert_eq!(result.spill, None);
}

#[tokio::test]
async fn caps_by_lines_from_either_end() {
    let dir = tempfile::tempdir().unwrap();
    let limits = OutputLimits {
        max_lines: 3,
        max_bytes: 10_000,
    };
    let head = truncate_output(numbered(10), limits, Keep::Head, dir.path(), "head").await;
    assert!(
        head.text
            .starts_with("line 1\nline 2\nline 3\n\n[Output truncated: 7 more lines")
    );
    let tail = truncate_output(numbered(10), limits, Keep::Tail, dir.path(), "tail").await;
    assert!(
        tail.text
            .starts_with("[Output truncated: the first 7 lines are not shown.")
    );
    assert!(tail.text.ends_with("]\n\nline 8\nline 9\nline 10\n"));
}

#[tokio::test]
async fn caps_by_bytes_and_clips_one_huge_line_on_a_char_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let limits = OutputLimits {
        max_lines: 1000,
        max_bytes: 20,
    };
    let head = truncate_output(numbered(100), limits, Keep::Head, dir.path(), "bytes").await;
    assert!(head.truncated);
    assert!(
        head.text
            .starts_with("line 1\nline 2\n\n[Output truncated: 98 more lines")
    );
    let narrow = OutputLimits {
        max_lines: 10,
        max_bytes: 7,
    };
    let huge = "é".repeat(50);
    let clipped = truncate_output(huge.clone(), narrow, Keep::Head, dir.path(), "wide").await;
    assert!(clipped.text.starts_with("ééé\n\n[Output truncated"));
    let tail = truncate_output(huge, narrow, Keep::Tail, dir.path(), "wide-tail").await;
    assert!(tail.text.ends_with("]\n\nééé"));
}

#[tokio::test]
async fn spills_full_text_to_data_dir() {
    let dir = tempfile::tempdir().unwrap();
    let text = numbered(50);
    let limits = OutputLimits {
        max_lines: 5,
        max_bytes: 10_000,
    };
    let result = truncate_output(text.clone(), limits, Keep::Head, dir.path(), "call/1").await;
    assert!(result.truncated);
    let spill = result.spill.clone().unwrap();
    assert_eq!(
        spill.file_name().and_then(|n| n.to_str()),
        Some("call_1.txt")
    );
    assert_eq!(std::fs::read_to_string(&spill).unwrap(), text);
    assert!(result.text.starts_with("line 1\n"));
    assert!(result.text.contains("45 more lines"));
}
