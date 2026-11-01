use std::io::{Cursor, Write};

use systemprompt_files::UploadLimits;
use systemprompt_files::uploads::RefusalReason;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::uploads_rig::{refusal, refusal_detail, rig, rig_with};

const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const MIB: usize = 1024 * 1024;

fn archive(entries: &[(&str, Vec<u8>, CompressionMethod)], large: bool) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content, method) in entries {
        let options = SimpleFileOptions::default()
            .compression_method(*method)
            .large_file(large);
        writer.start_file(*name, options).expect("start");
        writer.write_all(content).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn macro_docx() -> Vec<u8> {
    archive(
        &[
            (
                "[Content_Types].xml",
                b"<Types/>".to_vec(),
                CompressionMethod::Deflated,
            ),
            (
                "word/document.xml",
                b"<w:document/>".to_vec(),
                CompressionMethod::Deflated,
            ),
            (
                "word/vbaProject.bin",
                vec![0xd0, 0xcf, 0x11, 0xe0],
                CompressionMethod::Stored,
            ),
        ],
        false,
    )
}

fn benign() -> Vec<u8> {
    archive(
        &[
            (
                "[Content_Types].xml",
                b"<Types/>".to_vec(),
                CompressionMethod::Stored,
            ),
            (
                "word/document.xml",
                b"<w:document>hello</w:document>".to_vec(),
                CompressionMethod::Deflated,
            ),
        ],
        false,
    )
}

fn bomb(bytes: usize) -> Vec<u8> {
    archive(
        &[(
            "word/media/bomb.bin",
            vec![0_u8; bytes],
            CompressionMethod::Deflated,
        )],
        false,
    )
}

fn eocd(bytes: &[u8]) -> usize {
    bytes
        .windows(4)
        .rposition(|window| window == b"PK\x05\x06")
        .expect("end record")
}

#[tokio::test]
async fn a_benign_docx_passes_the_zip_guard() {
    let completed = rig().await.put(DOCX, &benign()).await.expect("benign");
    assert_eq!(completed.checked_kind, "zip");
    assert_eq!(completed.detected_mime, DOCX);
    assert_eq!(completed.size_bytes, benign().len() as u64);
}

#[tokio::test]
async fn a_zip64_archive_is_read_through_its_extra_fields() {
    let bytes = archive(
        &[(
            "word/document.xml",
            b"<w:document/>".to_vec(),
            CompressionMethod::Deflated,
        )],
        true,
    );
    let completed = rig().await.put(DOCX, &bytes).await.expect("zip64");
    assert_eq!(completed.checked_kind, "zip");
}

#[tokio::test]
async fn a_macro_bearing_docx_is_still_checked_as_an_archive() {
    let completed = rig()
        .await
        .put(DOCX, &macro_docx())
        .await
        .expect("structurally sound");
    assert_eq!(completed.checked_kind, "zip");
}

#[tokio::test]
async fn a_high_ratio_entry_is_refused_as_a_zip_bomb() {
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, &bomb(4 * MIB)).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(detail.contains("ratio"), "{detail}");
}

#[tokio::test]
async fn a_zip_bomb_declared_as_opaque_bytes_still_meets_the_guard() {
    let result = rig()
        .await
        .put("application/octet-stream", &bomb(4 * MIB))
        .await;
    assert_eq!(refusal(result), RefusalReason::ArchiveLimits);
}

#[tokio::test]
async fn entries_expanding_past_the_total_limit_are_refused() {
    let limits = UploadLimits {
        office_max_uncompressed_bytes: 64,
        ..UploadLimits::default()
    };
    let stored = archive(
        &[("a.bin", vec![7_u8; 200], CompressionMethod::Stored)],
        false,
    );
    let deflated = archive(
        &[("a.bin", vec![7_u8; 200], CompressionMethod::Deflated)],
        false,
    );
    let rig = rig_with(limits).await;
    for bytes in [stored, deflated] {
        let (reason, detail) = refusal_detail(rig.put(DOCX, &bytes).await);
        assert_eq!(reason, RefusalReason::ArchiveLimits);
        assert!(detail.contains("decompressed limit"), "{detail}");
    }
}

#[tokio::test]
async fn more_entries_than_allowed_are_refused() {
    let limits = UploadLimits {
        office_max_entries: 2,
        ..UploadLimits::default()
    };
    let (reason, detail) = refusal_detail(rig_with(limits).await.put(DOCX, &macro_docx()).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(detail.contains("entry limit"), "{detail}");
}

#[tokio::test]
async fn an_encrypted_entry_is_refused() {
    let mut bytes = benign();
    bytes[6] |= 0x01;
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("encrypted"), "{detail}");
}

#[tokio::test]
async fn an_unsupported_compression_method_is_refused() {
    let mut bytes = benign();
    bytes[8] = 12;
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("compression method 12"), "{detail}");
}

#[tokio::test]
async fn a_stored_entry_with_a_data_descriptor_is_refused() {
    let mut bytes = benign();
    bytes[6] |= 0x08;
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("unknown length"), "{detail}");
}

#[tokio::test]
async fn trailing_bytes_after_the_end_record_are_refused() {
    let mut bytes = benign();
    bytes.extend_from_slice(b"smuggled");
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("no end-of-central-directory"), "{detail}");
}

#[tokio::test]
async fn a_second_end_record_hidden_in_the_comment_is_refused() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("word/document.xml", SimpleFileOptions::default())
        .expect("start");
    writer.write_all(b"<w:document/>").expect("write");
    let mut fake = b"PK\x05\x06".to_vec();
    fake.extend_from_slice(&[0; 18]);
    writer
        .set_raw_comment(fake.into_boxed_slice())
        .expect("comment");
    let bytes = writer.finish().expect("finish").into_inner();
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("more than one"), "{detail}");
}

#[tokio::test]
async fn a_multi_disk_archive_is_refused() {
    let mut bytes = benign();
    let at = eocd(&bytes);
    bytes[at + 4] = 1;
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("multi-disk"), "{detail}");
}

#[tokio::test]
async fn a_directory_that_does_not_reach_the_end_record_is_refused() {
    let mut bytes = benign();
    let at = eocd(&bytes);
    bytes[at + 12] = bytes[at + 12].wrapping_sub(1);
    let (_, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert!(detail.contains("does not end where"), "{detail}");
}

#[tokio::test]
async fn a_local_entry_off_its_directory_offset_is_refused() {
    let mut bytes = benign();
    let central = bytes
        .windows(4)
        .rposition(|window| window == b"PK\x01\x02")
        .expect("last central header");
    bytes[central + 42] = bytes[central + 42].wrapping_add(1);
    let (reason, _) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
}

#[tokio::test]
async fn an_office_declaration_over_non_zip_bytes_is_a_mime_mismatch() {
    let result = rig().await.put(DOCX, b"%PDF-1.7 pretending").await;
    assert_eq!(refusal(result), RefusalReason::MimeMismatch);
}
