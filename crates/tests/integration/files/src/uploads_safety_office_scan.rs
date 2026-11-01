use std::io::{Cursor, Write};

use systemprompt_files::UploadLimits;
use systemprompt_files::uploads::RefusalReason;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::uploads_rig::{refusal_detail, rig, rig_with};

const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

fn deflated(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in entries {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file(*name, options).expect("start");
        writer.write_all(content).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn streamed(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = ZipWriter::new_stream(Vec::new());
    for (name, content) in entries {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file(*name, options).expect("start");
        writer.write_all(content).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn position(bytes: &[u8], signature: &[u8; 4]) -> usize {
    bytes
        .windows(4)
        .position(|window| window == signature)
        .expect("signature")
}

fn last(bytes: &[u8], signature: &[u8; 4]) -> usize {
    bytes
        .windows(4)
        .rposition(|window| window == signature)
        .expect("signature")
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn write_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn with_gap(bytes: &[u8], gap: &[u8]) -> Vec<u8> {
    let central = position(bytes, b"PK\x01\x02");
    let mut out = bytes[..central].to_vec();
    out.extend_from_slice(gap);
    out.extend_from_slice(&bytes[central..]);
    let eocd = last(&out, b"PK\x05\x06");
    let offset = read_u32(&out, eocd + 16);
    let shift = u32::try_from(gap.len()).expect("gap");
    write_u32(&mut out, eocd + 16, offset + shift);
    out
}

fn document() -> Vec<(&'static str, Vec<u8>)> {
    vec![(
        "word/document.xml",
        b"<w:document>hello</w:document>".to_vec(),
    )]
}

#[tokio::test]
async fn a_streamed_archive_with_data_descriptors_passes_the_guard() {
    let bytes = streamed(&[
        ("[Content_Types].xml", b"<Types/>".to_vec()),
        (
            "word/document.xml",
            b"<w:document>streamed</w:document>".to_vec(),
        ),
    ]);
    assert_ne!(u16::from_le_bytes([bytes[6], bytes[7]]) & 0x0008, 0);
    let completed = rig().await.put(DOCX, &bytes).await.expect("streamed");
    assert_eq!(completed.checked_kind, "zip");
    assert_eq!(completed.size_bytes, bytes.len() as u64);
}

#[tokio::test]
async fn gap_bytes_before_the_central_directory_are_refused() {
    let bytes = with_gap(&deflated(&document()), b"smuggled");
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(detail.contains("no entry accounts for"), "{detail}");
}

#[tokio::test]
async fn a_stray_central_header_before_the_directory_is_refused() {
    let bytes = with_gap(&deflated(&document()), b"PK\x01\x02rest");
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(detail.contains("do not end where"), "{detail}");
}

#[tokio::test]
async fn a_directory_that_misstates_the_compressed_size_is_refused() {
    let mut bytes = deflated(&document());
    let central = position(&bytes, b"PK\x01\x02");
    let listed = read_u32(&bytes, central + 20);
    write_u32(&mut bytes, central + 20, listed + 3);
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(
        detail.contains(&format!("not the {} its directory lists", listed + 3)),
        "{detail}"
    );
}

#[tokio::test]
async fn a_corrupt_deflate_stream_is_refused_as_unreadable() {
    let mut bytes = deflated(&document());
    let name_len = usize::from(u16::from_le_bytes([bytes[26], bytes[27]]));
    let extra_len = usize::from(u16::from_le_bytes([bytes[28], bytes[29]]));
    let data = 30 + name_len + extra_len;
    bytes[data] = 0xff;
    bytes[data + 1] = 0xff;
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(detail.contains("could not be read"), "{detail}");
}

#[tokio::test]
async fn small_entries_that_together_pass_the_ratio_are_refused_as_a_whole() {
    let entries: Vec<(&str, Vec<u8>)> = ["a.bin", "b.bin", "c.bin"]
        .into_iter()
        .map(|name| (name, vec![0_u8; 900 * 1024]))
        .collect();
    let bytes = deflated(&entries);
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, &bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits);
    assert!(detail.contains("the archive expands"), "{detail}");
}

#[tokio::test]
async fn small_entries_under_a_generous_ratio_pass() {
    let limits = UploadLimits {
        office_max_ratio: 1_000_000,
        ..UploadLimits::default()
    };
    let entries: Vec<(&str, Vec<u8>)> = ["a.bin", "b.bin"]
        .into_iter()
        .map(|name| (name, vec![0_u8; 900 * 1024]))
        .collect();
    let completed = rig_with(limits)
        .await
        .put(DOCX, &deflated(&entries))
        .await
        .expect("within ratio");
    assert_eq!(completed.checked_kind, "zip");
}
