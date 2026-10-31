use std::io::{Cursor, Write};

use systemprompt_files::uploads::RefusalReason;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::uploads_rig::{refusal_detail, rig};

const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

fn archive(large: bool, names: &[&str]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for name in names {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(large);
        writer.start_file(*name, options).expect("start");
        writer
            .write_all(b"<w:document>hello</w:document>")
            .expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn last(bytes: &[u8], signature: &[u8; 4]) -> usize {
    bytes
        .windows(4)
        .rposition(|window| window == signature)
        .expect("signature")
}

async fn refused(bytes: &[u8]) -> String {
    let (reason, detail) = refusal_detail(rig().await.put(DOCX, bytes).await);
    assert_eq!(reason, RefusalReason::ArchiveLimits, "{detail}");
    detail
}

#[tokio::test]
async fn an_end_record_claiming_zip64_without_a_locator_is_refused() {
    let mut bytes = archive(false, &["word/document.xml"]);
    let at = last(&bytes, b"PK\x05\x06");
    bytes[at + 10] = 0xff;
    bytes[at + 11] = 0xff;
    let detail = refused(&bytes).await;
    assert!(detail.contains("zip64 locator is missing"), "{detail}");
}

fn with_zip64_trailer(bytes: &[u8]) -> Vec<u8> {
    let at = last(bytes, b"PK\x05\x06");
    let field = |offset: usize, len: usize| {
        bytes[at + offset..at + offset + len]
            .iter()
            .rev()
            .fold(0_u64, |acc, byte| (acc << 8) | u64::from(*byte))
    };
    let (entries, size, offset) = (field(10, 2), field(12, 4), field(16, 4));
    let mut out = bytes[..at].to_vec();
    out.extend_from_slice(b"PK\x06\x06");
    out.extend_from_slice(&44_u64.to_le_bytes());
    out.extend_from_slice(&[45, 0, 45, 0]);
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&entries.to_le_bytes());
    out.extend_from_slice(&entries.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(b"PK\x06\x07");
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(at as u64).to_le_bytes());
    out.extend_from_slice(&1_u32.to_le_bytes());
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&[0xff; 4]);
    out.extend_from_slice(&[0xff; 8]);
    out.extend_from_slice(&[0, 0]);
    out
}

#[tokio::test]
async fn a_zip64_end_record_is_found_through_its_locator_and_checked() {
    let zip64 = with_zip64_trailer(&archive(false, &["word/document.xml"]));
    let completed = rig().await.put(DOCX, &zip64).await.expect("zip64 end");
    assert_eq!(completed.checked_kind, "zip");

    let mut moved = zip64.clone();
    let locator = last(&moved, b"PK\x06\x07");
    moved[locator + 8] = moved[locator + 8].wrapping_add(1);
    let detail = refused(&moved).await;
    assert!(detail.contains("not where its locator says"), "{detail}");

    let mut malformed = zip64.clone();
    let record = last(&malformed, b"PK\x06\x06");
    malformed[record + 4] = 45;
    let detail = refused(&malformed).await;
    assert!(detail.contains("zip64 end record is malformed"), "{detail}");

    let mut unsigned = zip64;
    let locator = last(&unsigned, b"PK\x06\x07");
    unsigned[locator + 3] = 0x09;
    let detail = refused(&unsigned).await;
    assert!(detail.contains("zip64 locator is missing"), "{detail}");
}

#[tokio::test]
async fn a_directory_listing_fewer_entries_than_it_holds_is_refused() {
    let mut bytes = archive(false, &["word/a.xml", "word/b.xml"]);
    let at = last(&bytes, b"PK\x05\x06");
    bytes[at + 8] = 1;
    bytes[at + 10] = 1;
    let detail = refused(&bytes).await;
    assert!(detail.contains("bytes no entry accounts for"), "{detail}");
}

#[tokio::test]
async fn a_directory_listing_more_entries_than_it_holds_is_malformed() {
    let mut bytes = archive(false, &["word/a.xml"]);
    let at = last(&bytes, b"PK\x05\x06");
    bytes[at + 8] = 2;
    bytes[at + 10] = 2;
    let detail = refused(&bytes).await;
    assert!(
        detail.contains("central directory is malformed"),
        "{detail}"
    );
}

#[tokio::test]
async fn a_zip64_sized_entry_without_its_extra_field_is_refused() {
    let mut bytes = archive(false, &["word/document.xml"]);
    let central = last(&bytes, b"PK\x01\x02");
    for byte in &mut bytes[central + 20..central + 24] {
        *byte = 0xff;
    }
    let detail = refused(&bytes).await;
    assert!(detail.contains("no zip64 extra field"), "{detail}");
}

#[tokio::test]
async fn a_corrupted_central_signature_is_malformed() {
    let mut bytes = archive(false, &["word/document.xml"]);
    let central = last(&bytes, b"PK\x01\x02");
    bytes[central + 3] = 0x09;
    let detail = refused(&bytes).await;
    assert!(
        detail.contains("central directory is malformed"),
        "{detail}"
    );
}
