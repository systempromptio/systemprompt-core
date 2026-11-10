use std::collections::BTreeMap;
use std::io::Write as _;

use chrono::{TimeZone, Utc};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use systemprompt_files::documents::model::layout::ImageWrap;
use systemprompt_files::documents::model::{Block, BlockKind, RichText};
use systemprompt_files::documents::pdf::{RenderRequest, render_rich};
use systemprompt_identifiers::DocumentRevisionId;

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(data.len())
        .expect("len")
        .to_be_bytes()
        .to_vec();
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
    out
}

fn gray_png(side: u32) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    let row = vec![0_u8; side as usize + 1];
    for _ in 0..side {
        encoder.write_all(&row).expect("deflate");
    }
    let mut ihdr = side.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&side.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(chunk(b"IHDR", &ihdr));
    bytes.extend(chunk(b"IDAT", &encoder.finish().expect("finish")));
    bytes.extend(chunk(b"IEND", &[]));
    bytes
}

fn pdf_with(picture: Vec<u8>) -> Vec<u8> {
    let model = RichText::of(vec![Block::new(
        "i0",
        BlockKind::Image {
            blob: "picture".to_owned(),
            alt: "the picture".to_owned(),
            width: None,
            height: None,
            caption: None,
            wrap: ImageWrap::Inline,
        },
        "",
    )]);
    let media = BTreeMap::from([("picture".to_owned(), picture)]);
    let revision = DocumentRevisionId::new("drev_01HZZZZZZZZZZZZZZZZZZZZZZZ");
    render_rich(
        &model,
        &media,
        &RenderRequest {
            name: "picture.md",
            revision: &revision,
            created_at: Utc
                .with_ymd_and_hms(2026, 11, 10, 9, 0, 0)
                .single()
                .expect("date"),
            max_pages: 5,
        },
    )
    .expect("render")
}

fn image_objects(pdf: &[u8]) -> usize {
    pdf.windows(b"/Subtype /Image".len())
        .filter(|window| *window == b"/Subtype /Image")
        .count()
}

#[test]
fn a_png_within_the_decode_budget_is_drawn() {
    assert_eq!(image_objects(&pdf_with(gray_png(1000))), 1);
}

#[test]
fn a_png_whose_pixels_exceed_the_decode_budget_is_not_decoded() {
    assert_eq!(image_objects(&pdf_with(gray_png(12_000))), 0);
}
