use std::fmt::Write as _;
use std::path::Path;

use serde_json::json;
use systemprompt_engine::tools::{ReadLimits, ReadTool, Tool, ToolError, ToolOutput};

use crate::tools::support::{invalid_message, put, recorded};

fn limits() -> ReadLimits {
    ReadLimits {
        default_lines: 2000,
        max_line_chars: 10,
        max_bytes: 1000,
        max_attachment_bytes: 1000,
    }
}

async fn read_at(
    root: &Path,
    path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<ToolOutput, ToolError> {
    let (ctx, _) = recorded(root);
    let mut input = json!({"path": path});
    if let Some(offset) = offset {
        input["offset"] = json!(offset);
    }
    if let Some(limit) = limit {
        input["limit"] = json!(limit);
    }
    ReadTool::new(limits()).run(input, &ctx).await
}

async fn read_text(
    content: &[u8],
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<ToolOutput, ToolError> {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "f.txt", content);
    read_at(dir.path(), "f.txt", offset, limit).await
}

#[tokio::test]
async fn numbers_lines_and_points_to_next_offset() {
    let out = read_text(b"a\nb\nc\n", None, Some(2)).await.unwrap();
    assert!(out.output.starts_with("     1\ta\n     2\tb\n"));
    assert!(out.output.contains("offset=3"));
    assert_eq!(out.metadata["more"], true);
    assert_eq!(
        (
            out.metadata["first_line"].clone(),
            out.metadata["last_line"].clone(),
            out.metadata["total_lines"].clone()
        ),
        (json!(1), json!(2), json!(3))
    );
}

#[tokio::test]
async fn offset_window_reaches_end() {
    let out = read_text(b"a\r\nb\r\nc", Some(2), Some(10)).await.unwrap();
    assert!(out.output.starts_with("     2\tb\n     3\tc\n"));
    assert!(out.output.contains("End of file: 3 lines"));
    assert_eq!(out.metadata["more"], false);
}

#[tokio::test]
async fn offset_past_end_is_an_error() {
    let err = read_text(b"a\n", Some(5), Some(10)).await.unwrap_err();
    assert!(invalid_message(&err).contains("past the end"));
}

#[tokio::test]
async fn clips_long_lines_on_char_boundaries_and_strips_bom() {
    let out = read_text("\u{feff}0123456789abcdef\n".as_bytes(), None, None)
        .await
        .unwrap();
    assert!(
        out.output
            .starts_with("     1\t0123456789… [line clipped]\n")
    );
    assert_eq!(out.metadata["clipped_lines"], 1);
    let accented = read_text("héllo wörld, again\n".as_bytes(), None, None)
        .await
        .unwrap();
    assert!(
        accented
            .output
            .starts_with("     1\théllo wörl… [line clipped]\n")
    );
}

#[tokio::test]
async fn byte_cap_stops_early() {
    let content: String = (0..500).fold(String::new(), |mut out, i| {
        writeln!(out, "line{i}").unwrap();
        out
    });
    let out = read_text(content.as_bytes(), None, None).await.unwrap();
    assert_eq!(out.metadata["more"], true);
    assert!(out.metadata["last_line"].as_u64().unwrap() < 500);
    assert!(out.output.contains("output size limit"));
}

#[tokio::test]
async fn empty_file_reads_as_empty() {
    let out = read_text(b"", None, None).await.unwrap();
    assert_eq!(out.output, "(empty file)");
}

#[tokio::test]
async fn utf16_files_are_decoded() {
    let out = read_text(&[0xFF, 0xFE, b'h', 0, b'i', 0], None, None)
        .await
        .unwrap();
    assert!(out.output.starts_with("     1\thi\n"));
}

#[tokio::test]
async fn binary_files_are_refused() {
    let err = read_text(b"PK\x03\x04\x00\x00", None, None)
        .await
        .unwrap_err();
    assert!(invalid_message(&err).contains("binary"));
}

#[tokio::test]
async fn images_and_pdfs_come_back_as_attachments() {
    let png = read_text(b"\x89PNG\r\n\x1a\n", None, None).await.unwrap();
    assert_eq!(png.attachments.len(), 1);
    assert_eq!(png.attachments[0].mime, "image/png");
    let pdf = read_text(b"%PDF-1.7", None, None).await.unwrap();
    assert_eq!(pdf.attachments[0].mime, "application/pdf");
    let text = read_text(b"hello", None, None).await.unwrap();
    assert_eq!(
        text.attachments,
        Vec::<systemprompt_engine::tools::Attachment>::new()
    );
}

#[tokio::test]
async fn suggests_similar_names() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "Config.toml", b"x");
    let err = read_at(dir.path(), "config.tml", None, None)
        .await
        .unwrap_err();
    let message = invalid_message(&err);
    assert!(message.contains("Did you mean") && message.contains("Config.toml"));
}

#[tokio::test]
async fn directories_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "src/a.rs", b"");
    put(dir.path(), "b.txt", b"");
    let out = read_at(dir.path(), ".", None, None).await.unwrap();
    assert!(out.output.contains("b.txt\nsrc/"));
    assert_eq!(out.metadata["directory"], true);
}
