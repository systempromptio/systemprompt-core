use systemprompt_files::UploadLimits;
use systemprompt_files::uploads::RefusalReason;

use crate::uploads_rig::{refusal, refusal_detail, rig, rig_with};

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = u32::try_from(data.len())
        .expect("len")
        .to_be_bytes()
        .to_vec();
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&[0; 4]);
    chunk
}

fn png(width: u32, height: u32, extra: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(png_chunk(b"IHDR", &ihdr));
    for (kind, data) in extra {
        bytes.extend(png_chunk(kind, data));
    }
    bytes.extend(png_chunk(b"IDAT", &[0x78, 0x9c, 0x63, 0, 0, 0, 1, 0, 1]));
    bytes.extend(png_chunk(b"IEND", &[]));
    bytes
}

fn jpeg(width: u16, height: u16, with_exif: bool) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xd8];
    if with_exif {
        bytes.extend_from_slice(&[0xff, 0xe1, 0x00, 0x08]);
        bytes.extend_from_slice(b"Exif\0\0");
        bytes.extend_from_slice(&[0xff, 0xfe, 0x00, 0x06]);
        bytes.extend_from_slice(b"gps!");
    }
    bytes.extend_from_slice(&[0xff, 0xc0, 0x00, 0x0b, 0x08]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
    bytes.extend_from_slice(&[0xff, 0xda, 0x00, 0x02, 0x00, 0xff, 0xd9]);
    bytes
}

fn gif(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = b"GIF89a".to_vec();
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0x3b]);
    bytes
}

fn riff_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = kind.to_vec();
    chunk.extend_from_slice(&u32::try_from(data.len()).expect("len").to_le_bytes());
    chunk.extend_from_slice(data);
    if data.len() % 2 == 1 {
        chunk.push(0);
    }
    chunk
}

fn webp(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&u32::try_from(body.len() + 4).expect("len").to_le_bytes());
    bytes.extend_from_slice(b"WEBP");
    bytes.extend(body);
    bytes
}

fn vp8x(width: u32, height: u32, flags: u8) -> Vec<u8> {
    let mut data = vec![flags, 0, 0, 0];
    data.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
    riff_chunk(b"VP8X", &data)
}

#[tokio::test]
async fn png_metadata_chunks_are_stripped_and_the_rest_kept() {
    let bytes = png(
        4,
        4,
        &[
            (b"tEXt", b"Author\0me"),
            (b"eXIf", b"MM\0*"),
            (b"tIME", &[0; 7]),
        ],
    );
    let completed = rig().await.put("image/png", &bytes).await.expect("png");
    assert_eq!(completed.checked_kind, "png");
    assert_eq!(completed.detected_mime, "image/png");
    assert_eq!(completed.size_bytes, png(4, 4, &[]).len() as u64);
}

#[tokio::test]
async fn a_clean_png_is_adopted_unchanged() {
    let bytes = png(2, 3, &[]);
    let completed = rig().await.put("image/png", &bytes).await.expect("png");
    assert_eq!(completed.size_bytes, bytes.len() as u64);
}

#[tokio::test]
async fn images_past_the_dimension_cap_are_refused() {
    let rig = rig().await;
    let oversized = [
        ("image/png", png(20_000, 1, &[])),
        ("image/png", png(0, 5, &[])),
        ("image/jpeg", jpeg(1, 40_000, false)),
        ("image/gif", gif(17_000, 1)),
        ("image/webp", webp(&[vp8x(16_385, 1, 0)])),
    ];
    for (mime, bytes) in oversized {
        let (reason, detail) = refusal_detail(rig.put(mime, &bytes).await);
        assert_eq!(reason, RefusalReason::ImageLimits, "{mime}");
        assert!(detail.contains("dimension cap"), "{mime}: {detail}");
    }
}

#[tokio::test]
async fn an_unreadable_image_header_is_refused() {
    let rig = rig().await;
    let truncated = [
        ("image/png", b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec()),
        ("image/jpeg", vec![0xff, 0xd8, 0xff, 0xe0, 0x00]),
        ("image/jpeg", vec![0xff, 0xd8, 0xff, 0xda, 0x00, 0x02]),
        ("image/gif", b"GIF89a\x01".to_vec()),
        ("image/webp", webp(&[riff_chunk(b"ICCP", b"xx")])),
    ];
    for (mime, bytes) in truncated {
        let (reason, detail) = refusal_detail(rig.put(mime, &bytes).await);
        assert_eq!(reason, RefusalReason::ImageLimits, "{mime}");
        assert!(
            detail.contains("header could not be read"),
            "{mime}: {detail}"
        );
    }
}

#[tokio::test]
async fn jpeg_exif_and_comment_segments_are_stripped() {
    let completed = rig()
        .await
        .put("image/jpeg", &jpeg(8, 6, true))
        .await
        .expect("jpeg");
    assert_eq!(completed.checked_kind, "jpeg");
    assert_eq!(completed.size_bytes, jpeg(8, 6, false).len() as u64);
}

#[tokio::test]
async fn a_gif_is_measured_and_kept() {
    let bytes = gif(10, 10);
    let completed = rig().await.put("image/gif", &bytes).await.expect("gif");
    assert_eq!(completed.checked_kind, "gif");
    assert_eq!(completed.size_bytes, bytes.len() as u64);
}

#[tokio::test]
async fn webp_metadata_chunks_and_flags_are_cleared() {
    let image = riff_chunk(b"VP8L", &[0x2f, 0x00, 0x00, 0x00, 0x00]);
    let tagged = webp(&[
        vp8x(3, 3, 0x0c),
        riff_chunk(b"EXIF", b"abc"),
        image.clone(),
        riff_chunk(b"XMP ", b"<x/>"),
    ]);
    let completed = rig().await.put("image/webp", &tagged).await.expect("webp");
    assert_eq!(completed.checked_kind, "webp");
    assert_eq!(
        completed.size_bytes,
        webp(&[vp8x(3, 3, 0), image]).len() as u64
    );
}

#[tokio::test]
async fn webp_lossless_and_lossy_frames_report_their_dimensions() {
    let rig = rig().await;
    let lossless = webp(&[riff_chunk(b"VP8L", &[0x2f, 0x07, 0x00, 0x00, 0x00])]);
    rig.put("image/webp", &lossless).await.expect("vp8l");
    let mut frame = vec![0; 10];
    frame[6..8].copy_from_slice(&5_u16.to_le_bytes());
    frame[8..10].copy_from_slice(&7_u16.to_le_bytes());
    rig.put("image/webp", &webp(&[riff_chunk(b"VP8 ", &frame)]))
        .await
        .expect("vp8");
    let mut zero = vec![0; 10];
    zero[8..10].copy_from_slice(&7_u16.to_le_bytes());
    let refused = rig
        .put("image/webp", &webp(&[riff_chunk(b"VP8 ", &zero)]))
        .await;
    assert_eq!(refusal(refused), RefusalReason::ImageLimits);
}

#[tokio::test]
async fn an_image_over_the_byte_limit_is_refused_before_it_is_read() {
    let limits = UploadLimits {
        image_max_bytes: 16,
        ..UploadLimits::default()
    };
    let (reason, detail) = refusal_detail(
        rig_with(limits)
            .await
            .put("image/png", &png(1, 1, &[]))
            .await,
    );
    assert_eq!(reason, RefusalReason::ImageLimits);
    assert!(detail.contains("byte limit"), "{detail}");
}

#[tokio::test]
async fn a_polyglot_declared_as_one_image_type_but_sniffing_as_another_is_refused() {
    let rig = rig().await;
    assert_eq!(
        refusal(rig.put("image/png", &gif(1, 1)).await),
        RefusalReason::MimeMismatch
    );
    assert_eq!(
        refusal(rig.put("image/jpeg", &png(1, 1, &[])).await),
        RefusalReason::MimeMismatch
    );
    assert_eq!(
        refusal(rig.put("image/gif", b"<svg></svg>").await),
        RefusalReason::MimeMismatch
    );
}

#[tokio::test]
async fn svg_with_active_content_is_refused_and_inert_svg_passes() {
    let rig = rig().await;
    let inert = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4"/></svg>"#;
    let completed = rig.put("image/svg+xml", inert).await.expect("inert svg");
    assert_eq!(completed.checked_kind, "svg");
    let scripted = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    assert_eq!(
        refusal(rig.put("image/svg+xml", scripted).await),
        RefusalReason::UnsafeSvg
    );
    let handler = br#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"/>"#;
    assert_eq!(
        refusal(rig.put("image/svg+xml", handler).await),
        RefusalReason::UnsafeSvg
    );
}

#[tokio::test]
async fn an_svg_over_its_byte_limit_is_refused() {
    let limits = UploadLimits {
        svg_max_bytes: 8,
        ..UploadLimits::default()
    };
    let result = rig_with(limits)
        .await
        .put("image/svg+xml", b"<svg xmlns=\"x\"/>")
        .await;
    assert_eq!(refusal(result), RefusalReason::UnsafeSvg);
}

#[tokio::test]
async fn text_and_opaque_uploads_record_the_check_their_bytes_call_for() {
    let rig = rig().await;
    let text = rig
        .put("application/x-notes", b"plain words")
        .await
        .expect("text");
    assert_eq!(text.checked_kind, "text");
    assert_eq!(text.detected_mime, "application/octet-stream");
    let opaque = rig
        .put("application/x-blob", &[0, 159, 146, 150])
        .await
        .expect("opaque");
    assert_eq!(opaque.checked_kind, "opaque");
    let pdf = rig
        .put("application/pdf", b"%PDF-1.7\n%%EOF")
        .await
        .expect("pdf");
    assert_eq!(pdf.checked_kind, "pdf");
    let markdown = rig
        .put("text/markdown; charset=utf-8", b"# hi")
        .await
        .expect("md");
    assert_eq!(markdown.checked_kind, "text");
}
