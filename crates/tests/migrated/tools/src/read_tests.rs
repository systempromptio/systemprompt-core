//! The read tool: content sniffing, paging, limits, attachments, permission
//! and what a missing path tells the model.

use std::fmt::Write as _;

use serde_json::{Value, json};
use systemprompt_engine::tools::{ReadLimits, ReadTool, ToolError};

use crate::support::{Called, Gate, Project};

fn limits() -> ReadLimits {
    ReadLimits {
        default_lines: 3,
        max_line_chars: 200,
        max_bytes: 4096,
        max_attachment_bytes: 64,
    }
}

async fn read(project: &Project, input: Value) -> Called {
    project.call(&ReadTool::new(limits()), input).await
}

async fn read_bytes(body: &[u8], name: &str) -> Called {
    let project = Project::new();
    project.put(name, body);
    let called = read(&project, json!({"path": name})).await;
    drop(project);
    called
}

fn numbered(lines: usize) -> String {
    (1..=lines).fold(String::new(), |mut out, index| {
        writeln!(out, "line {index}").unwrap();
        out
    })
}

#[tokio::test]
async fn text_starting_with_the_bitmap_letters_stays_text() {
    let out = read_bytes(b"BM is a pair of letters\n", "notes.txt")
        .await
        .ok();
    assert!(out.attachments.is_empty());
    assert!(out.output.contains("BM is a pair of letters"));
}

#[tokio::test]
async fn a_bitmap_is_recognised_from_its_header() {
    let mut bmp = b"BM".to_vec();
    bmp.extend_from_slice(&[0x3a, 0, 0, 0, 0, 0, 0, 0, 0x36, 0, 0, 0]);
    let out = read_bytes(&bmp, "picture").await.ok();
    assert_eq!(out.attachments[0].mime, "image/bmp");
}

#[tokio::test]
#[ignore = "parity ledger: a PDF header after leading bytes is not recognised"]
async fn a_pdf_header_within_the_first_kilobyte_is_recognised() {
    let mut body = b"\n\n   ".to_vec();
    body.extend_from_slice(b"%PDF-1.7\n");
    let out = read_bytes(&body, "doc").await.ok();
    assert_eq!(
        out.attachments.first().map(|a| a.mime.as_str()),
        Some("application/pdf")
    );
}

#[tokio::test]
async fn an_archive_that_contains_a_pdf_is_not_taken_for_one() {
    let mut zip = b"PK\x03\x04\x14\x00\x00\x00".to_vec();
    zip.extend_from_slice(b"inner.pdf%PDF-1.4");
    let err = read_bytes(&zip, "bundle.zip").await.message();
    assert!(err.contains("binary"), "{err}");
}

#[tokio::test]
async fn text_that_mentions_a_pdf_header_stays_text() {
    let out = read_bytes(b"Files begin with %PDF-1.4 as a rule.\n", "notes.md")
        .await
        .ok();
    assert!(out.attachments.is_empty());
    assert!(out.output.contains("%PDF-1.4"));
}

#[tokio::test]
#[ignore = "parity ledger: media containers are not told apart by brand"]
async fn media_container_brands_are_told_apart() {
    let avif = b"\x00\x00\x00\x1cftypavif\x00\x00\x00\x00avifmif1";
    let out = read_bytes(avif, "image.bin").await.ok();
    assert_eq!(
        out.attachments.first().map(|a| a.mime.as_str()),
        Some("image/avif")
    );
}

#[tokio::test]
async fn the_default_line_limit_stops_at_whole_lines_and_says_where_to_continue() {
    let out = read_bytes(numbered(5).as_bytes(), "f.txt").await.ok();
    assert!(
        out.output
            .starts_with("     1\tline 1\n     2\tline 2\n     3\tline 3\n")
    );
    assert!(
        out.output
            .contains("line limit reached. Use offset=4 to continue.")
    );
}

#[tokio::test]
async fn a_gif_without_its_extension_is_read_as_an_image() {
    let out = read_bytes(b"GIF89a\x01\x00\x01\x00", "renamed.txt")
        .await
        .ok();
    assert_eq!(out.attachments[0].mime, "image/gif");
    assert_eq!(out.attachments[0].filename.as_deref(), Some("renamed.txt"));
}

#[tokio::test]
async fn malformed_utf8_is_read_lossily_but_nul_bytes_mean_binary() {
    let out = read_bytes(b"caf\xe9 ok\n", "latin.txt").await.ok();
    assert!(out.output.starts_with("     1\tcaf\u{fffd} ok\n"));
    let err = read_bytes(b"text\x00more\n", "nul.txt").await.message();
    assert!(err.contains("binary"), "{err}");
}

#[tokio::test]
async fn text_with_a_binary_looking_extension_is_read_as_text() {
    let out = read_bytes(b"plain words\n", "program.exe").await.ok();
    assert!(out.output.contains("plain words"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_listing_shows_broken_and_escaping_symlinks_without_following_them() {
    let project = Project::new();
    let outside = project.outside();
    project.put("dir/real.txt", b"x");
    std::os::unix::fs::symlink(project.root.join("absent"), project.root.join("dir/broken"))
        .unwrap();
    std::os::unix::fs::symlink(&outside, project.root.join("dir/escape")).unwrap();
    let out = read(&project, json!({"path": "dir"})).await.ok();
    assert!(out.output.contains("broken\n"), "{}", out.output);
    assert!(out.output.contains("escape"), "{}", out.output);
    assert!(out.output.contains("real.txt"));
    assert_eq!(out.metadata["entries"], 3);
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_directory_is_read_as_a_listing() {
    let project = Project::new();
    project.put("real/a.txt", b"x");
    std::os::unix::fs::symlink(project.root.join("real"), project.root.join("alias")).unwrap();
    let out = read(&project, json!({"path": "alias"})).await.ok();
    assert_eq!(out.metadata["directory"], true);
    assert!(out.output.contains("a.txt"));
}

#[tokio::test]
#[ignore = "parity ledger: an offset past the end is an error, not the file's last lines"]
async fn an_offset_past_the_end_returns_the_last_lines() {
    let project = Project::new();
    project.put("f.txt", numbered(10).as_bytes());
    let past = read(&project, json!({"path": "f.txt", "offset": 50}))
        .await
        .ok();
    assert!(past.output.contains("line 10"));
    project.put("big.txt", numbered(40_000).as_bytes());
    let big = read(&project, json!({"path": "big.txt", "offset": 90_000}))
        .await
        .ok();
    assert!(big.output.contains("line 40000"));
}

#[tokio::test]
#[ignore = "parity ledger: negative offsets counted from the end are refused"]
async fn a_negative_offset_starts_that_many_lines_before_the_end() {
    let project = Project::new();
    project.put("f.txt", numbered(10).as_bytes());
    let out = read(&project, json!({"path": "f.txt", "offset": -2}))
        .await
        .ok();
    assert!(out.output.starts_with("     9\tline 9\n    10\tline 10\n"));
}

#[tokio::test]
async fn the_end_of_a_long_file_is_read_with_real_line_numbers() {
    let project = Project::new();
    project.put("big.txt", numbered(60_000).as_bytes());
    let out = read(&project, json!({"path": "big.txt", "offset": 59_999}))
        .await
        .ok();
    assert!(
        out.output
            .starts_with(" 59999\tline 59999\n 60000\tline 60000\n")
    );
    assert!(out.output.contains("End of file: 60000 lines"));
    let page = read(
        &project,
        json!({"path": "big.txt", "offset": 30_000, "limit": 1}),
    )
    .await
    .ok();
    assert!(page.output.starts_with(" 30000\tline 30000\n"));
    assert!(page.output.contains("offset=30001"));
}

#[tokio::test]
async fn a_negative_offset_for_a_directory_is_an_error() {
    let project = Project::new();
    project.put("dir/a.txt", b"x");
    let err = read(&project, json!({"path": "dir", "offset": -1}))
        .await
        .err();
    assert!(matches!(err, ToolError::InvalidInput { .. }), "{err:?}");
}

#[tokio::test]
async fn directory_entries_are_sorted_without_regard_to_case() {
    let project = Project::new();
    for name in ["b.txt", "A.txt", "c/x.txt", "B2.txt"] {
        project.put(&format!("dir/{name}"), b"");
    }
    let out = read(&project, json!({"path": "dir"})).await.ok();
    assert!(
        out.output.contains("A.txt\nb.txt\nB2.txt\nc/"),
        "{}",
        out.output
    );
}

#[tokio::test]
#[ignore = "parity ledger: directory listings ignore offset and limit"]
async fn directory_listings_page_with_offset_and_limit() {
    let project = Project::new();
    for name in ["a", "b", "c", "d"] {
        project.put(&format!("dir/{name}.txt"), b"");
    }
    let out = read(&project, json!({"path": "dir", "offset": 2, "limit": 2}))
        .await
        .ok();
    assert!(out.output.contains("b.txt\nc.txt"));
    assert!(!out.output.contains("a.txt"));
    assert!(!out.output.contains("d.txt"));
}

#[tokio::test]
#[ignore = "parity ledger: a NUL byte after the requested page still makes the file binary"]
async fn nul_bytes_after_the_requested_page_do_not_matter() {
    let project = Project::new();
    project.put("f.txt", b"first\nsecond\n\x00tail\n");
    let out = read(&project, json!({"path": "f.txt", "limit": 1}))
        .await
        .ok();
    assert!(out.output.starts_with("     1\tfirst\n"));
}

#[tokio::test]
async fn nul_bytes_in_skipped_lines_still_mean_binary() {
    let project = Project::new();
    project.put("f.txt", b"\x00head\nsecond\nthird\n");
    let err = read(&project, json!({"path": "f.txt", "offset": 3}))
        .await
        .message();
    assert!(err.contains("binary"), "{err}");
}

#[tokio::test]
#[ignore = "parity ledger: files above 64 MiB are refused instead of paged"]
async fn a_very_large_file_is_paged_without_reading_it_whole() {
    let project = Project::new();
    let path = project.put("huge.txt", b"first line\n");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(65 * 1024 * 1024)
        .unwrap();
    let out = read(&project, json!({"path": "huge.txt", "limit": 1}))
        .await
        .ok();
    assert!(out.output.starts_with("     1\tfirst line\n"));
}

#[tokio::test]
async fn media_over_the_attachment_limit_is_refused_with_the_limit_named() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(65, 0);
    let err = read_bytes(&png, "big.png").await.message();
    assert!(err.contains("over the 64 byte attachment limit"), "{err}");
    let mut pdf = b"%PDF-1.7\n".to_vec();
    pdf.resize(65, b' ');
    let err = read_bytes(&pdf, "big.pdf").await.message();
    assert!(err.contains("application/pdf of 65 bytes"), "{err}");
}

#[tokio::test]
async fn media_exactly_at_the_attachment_limit_is_attached() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(64, 0);
    let out = read_bytes(&png, "edge.png").await.ok();
    assert_eq!(out.attachments.len(), 1);
    assert_eq!(out.metadata["bytes"], 64);
}

#[tokio::test]
async fn a_png_larger_than_the_text_limit_is_still_attached() {
    let project = Project::new();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(6000, 7);
    project.put("shot.png", &png);
    let tool = ReadTool::new(ReadLimits {
        max_bytes: 100,
        ..ReadLimits::default()
    });
    let out = project.call(&tool, json!({"path": "shot.png"})).await.ok();
    assert_eq!(out.attachments[0].mime, "image/png");
    assert_eq!(out.metadata["bytes"], 6000);
}

#[tokio::test]
#[ignore = "parity ledger: image data that cannot be decoded is attached as it is"]
async fn undecodable_image_data_is_dropped_with_an_explanation() {
    let out = read_bytes(b"\x89PNG\r\n\x1a\nnot really an image", "bad.png")
        .await
        .ok();
    assert!(out.attachments.is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: images over the limit are refused, not resized or dropped with a note"]
async fn an_image_over_the_limit_is_resized_or_dropped_with_an_explanation() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(65, 0);
    let out = read_bytes(&png, "big.png").await.ok();
    assert!(out.attachments.is_empty());
    assert!(out.output.contains("big.png"));
}

#[tokio::test]
async fn a_read_asks_read_permission_for_the_relative_path() {
    let project = Project::new();
    project.put("src/a.txt", b"x\n");
    let called = read(&project, json!({"path": "src/a.txt"})).await;
    let asked = called.gate.asked();
    called.ok();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].permission, "read");
    assert_eq!(asked[0].patterns, ["src/a.txt"]);
}

#[tokio::test]
async fn a_zero_limit_is_refused_before_the_disk_is_touched() {
    let project = Project::new();
    let called = read(&project, json!({"path": "absent.txt", "limit": 0})).await;
    assert!(called.gate.asked().is_empty());
    let err = called.message();
    assert!(err.contains("`limit` must be at least 1"), "{err}");
}

#[tokio::test]
async fn an_outside_absolute_path_asks_outside_approval_before_read() {
    let project = Project::new();
    let file = crate::support::put(&project.outside(), "o.txt", b"x\n");
    let called = read(&project, json!({"path": file.to_string_lossy()})).await;
    assert_eq!(called.gate.permissions(), ["external_directory", "read"]);
    assert!(called.ok().output.contains("\tx"));
}

#[tokio::test]
async fn nothing_is_read_or_listed_when_read_is_refused() {
    let project = Project::new();
    project.put("secret.txt", b"secret\n");
    project.put("dir/a.txt", b"");
    for path in ["secret.txt", "dir"] {
        let called = project
            .call_with(
                &ReadTool::new(limits()),
                json!({"path": path}),
                Gate::refusing("read"),
            )
            .await;
        assert!(matches!(called.err(), ToolError::Rejected));
    }
}

#[tokio::test]
async fn nothing_is_read_when_outside_approval_is_refused() {
    let project = Project::new();
    let file = crate::support::put(&project.outside(), "o.txt", b"x\n");
    let called = project
        .call_with(
            &ReadTool::new(limits()),
            json!({"path": file.to_string_lossy()}),
            Gate::refusing("external_directory"),
        )
        .await;
    assert_eq!(called.gate.permissions(), ["external_directory"]);
    assert!(matches!(called.err(), ToolError::Rejected));
}

#[tokio::test]
async fn a_missing_path_is_a_failure_the_model_can_read() {
    let project = Project::new();
    let err = read(&project, json!({"path": "nope.txt"})).await.err();
    match err {
        ToolError::InvalidInput { message } => assert_eq!(message, "file not found: nope.txt"),
        other => panic!("expected invalid input, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "parity ledger: a missing path does not name its first missing folder"]
async fn a_missing_path_names_its_first_missing_folder_and_nearest_parent() {
    let project = Project::new();
    project.put("src/lib.rs", b"");
    let err = read(&project, json!({"path": "src/gone/deeper/a.rs"}))
        .await
        .message();
    assert!(err.to_lowercase().contains("folder"), "{err}");
    assert!(err.contains("src/gone"), "{err}");
}

#[tokio::test]
async fn at_most_three_same_folder_suggestions_are_offered() {
    let project = Project::new();
    for name in ["config.toml", "config.yaml", "config.json", "config.ini"] {
        project.put(name, b"");
    }
    let err = read(&project, json!({"path": "config.txt"}))
        .await
        .message();
    let listed = err.split("Did you mean: ").nth(1).unwrap_or_default();
    assert_eq!(listed.matches("config.").count(), 3, "{err}");
}

#[tokio::test]
#[ignore = "parity ledger: a name differing only in unusual spaces is not recovered"]
async fn a_unique_name_differing_only_in_unusual_spaces_is_read() {
    let project = Project::new();
    project.put("my\u{a0}notes.txt", b"found\n");
    let out = read(&project, json!({"path": "my notes.txt"})).await.ok();
    assert!(out.output.contains("found"));
}

#[tokio::test]
async fn an_ambiguous_near_match_or_a_directory_is_never_read_instead() {
    let project = Project::new();
    project.put("my\u{a0}notes.txt", b"one\n");
    project.put("my\u{2009}notes.txt", b"two\n");
    project.put("my\u{a0}dir/a.txt", b"");
    for path in ["my notes.txt", "my dir"] {
        let err = read(&project, json!({"path": path})).await.err();
        assert!(matches!(err, ToolError::InvalidInput { .. }), "{err:?}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_file_is_reported_with_an_actionable_message() {
    use std::os::unix::fs::PermissionsExt as _;
    let project = Project::new();
    let path = project.put("locked.txt", b"x");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&path).is_ok() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        // skip-ok: running as a user that bypasses file modes (root) cannot observe the
        // failure
        return;
    }
    let err = read(&project, json!({"path": "locked.txt"}))
        .await
        .message();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(err.contains("could not read locked.txt"), "{err}");
}

#[tokio::test]
async fn a_large_directory_is_listed_in_a_bounded_page() {
    let project = Project::new();
    for index in 0..510 {
        project.put(&format!("many/f{index:03}.txt"), b"");
    }
    let out = read(&project, json!({"path": "many"})).await.ok();
    assert_eq!(out.metadata["entries"], 510);
    assert!(
        out.output
            .contains("… and 10 more. Use glob to narrow the listing.")
    );
    assert!(!out.output.contains("f509.txt"));
}

#[tokio::test]
#[ignore = "parity ledger: a zero offset is refused rather than read as the first page"]
async fn a_zero_directory_offset_reads_as_the_first_page() {
    let project = Project::new();
    project.put("dir/a.txt", b"");
    let out = read(&project, json!({"path": "dir", "offset": 0}))
        .await
        .ok();
    assert!(out.output.contains("a.txt"));
}
