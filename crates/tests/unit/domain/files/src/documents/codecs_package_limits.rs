use std::io::{Cursor, Write};

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::docx::{DocxCodec, blank_docx};
use systemprompt_files::documents::codecs::{PackageCaps, inspect_package};
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::docx_fixture::entries;

const MIB: usize = 1024 * 1024;

const SMALL: PackageCaps = PackageCaps {
    part_bytes: 1024 * 1024,
    inflated_bytes: 2 * 1024 * 1024,
    parts: 64,
};

fn docx_with(extra: &[(String, usize)]) -> Vec<u8> {
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content, ..) in entries(&blank_docx().expect("blank")) {
        writer.start_file(name, deflated).expect("start");
        writer.write_all(&content).expect("write");
    }
    for (name, size) in extra {
        writer.start_file(name.as_str(), deflated).expect("start");
        writer.write_all(&vec![0_u8; *size]).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn too_large(result: Result<(), CodecError>) -> (&'static str, u64, u64) {
    match result {
        Err(CodecError::TooLarge { what, size, limit }) => (what, size, limit),
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

#[test]
fn a_blank_docx_is_within_small_caps() {
    inspect_package(&blank_docx().expect("blank"), SMALL).expect("within caps");
}

#[test]
fn plain_bytes_that_are_not_a_zip_pass_through() {
    inspect_package(b"# plain markdown\n\nPK is not a signature.", SMALL).expect("not a package");
}

#[test]
fn a_bomb_behind_a_prepended_byte_is_still_too_large() {
    let mut prefixed = vec![b'#'];
    prefixed.extend(docx_with(&[("word/media/bomb.bin".to_owned(), 2 * MIB)]));
    let (what, _, limit) = too_large(inspect_package(&prefixed, SMALL));
    assert_eq!(what, "zip package part (inflated bytes)");
    assert_eq!(limit, SMALL.part_bytes);
}

#[test]
fn bytes_with_a_zip_signature_that_do_not_open_are_refused() {
    let mut broken = b"PK\x03\x04".to_vec();
    broken.extend_from_slice(&[0_u8; 32]);
    assert!(matches!(
        inspect_package(&broken, SMALL),
        Err(CodecError::Parse { .. })
    ));
}

#[test]
fn a_part_that_inflates_past_its_cap_is_too_large() {
    let bomb = docx_with(&[("word/media/bomb.bin".to_owned(), 2 * MIB)]);
    assert!(
        bomb.len() < 64 * 1024,
        "the fixture must stay small compressed"
    );
    let (what, size, limit) = too_large(inspect_package(&bomb, SMALL));
    assert_eq!(what, "zip package part (inflated bytes)");
    assert_eq!(limit, SMALL.part_bytes);
    assert_eq!(size, SMALL.part_bytes + 1);
}

#[test]
fn parts_that_inflate_past_the_total_cap_together_are_too_large() {
    let parts: Vec<(String, usize)> = (0..3)
        .map(|n| (format!("word/media/fill{n}.bin"), 900 * 1024))
        .collect();
    let (what, _, limit) = too_large(inspect_package(&docx_with(&parts), SMALL));
    assert_eq!(what, "zip package (inflated bytes)");
    assert_eq!(limit, SMALL.inflated_bytes);
}

#[test]
fn a_package_listing_too_many_parts_is_too_large() {
    let parts: Vec<(String, usize)> = (0..80)
        .map(|n| (format!("customXml/item{n}.xml"), 1))
        .collect();
    let (what, size, limit) = too_large(inspect_package(&docx_with(&parts), SMALL));
    assert_eq!(what, "zip package parts");
    assert_eq!(limit, SMALL.parts);
    assert!(size > SMALL.parts);
}

#[test]
fn the_codec_ceiling_is_the_configurable_maximum() {
    assert!(PackageCaps::CEILING.part_bytes >= SMALL.part_bytes);
    assert!(PackageCaps::CEILING.inflated_bytes >= PackageCaps::CEILING.part_bytes);
}

#[test]
fn a_bomb_still_projects_when_it_fits_under_the_codec_ceiling() {
    let bomb = docx_with(&[("word/media/bomb.bin".to_owned(), 2 * MIB)]);
    DocxCodec
        .project(&bomb)
        .expect("ceiling is far above 2 MiB");
}

#[test]
fn a_codec_too_large_becomes_the_documents_too_large_refusal() {
    let error = DocumentsError::from(CodecError::TooLarge {
        what: "zip package parts",
        size: 9,
        limit: 8,
    });
    assert!(matches!(
        error,
        DocumentsError::TooLarge {
            what: "zip package parts",
            size: 9,
            limit: 8,
        }
    ));
}
