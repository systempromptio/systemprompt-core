use systemprompt_files::documents::codecs::docx::{
    MAX_PART_BYTES, blank_docx, held_image, import_rich, materialise_rich_with,
};
use systemprompt_files::documents::model::layout::ImageWrap;
use systemprompt_files::documents::model::{Block, BlockKind, Media, MediaBlob, RichText};
use systemprompt_provider_contracts::documents::content_hash;

use super::docx_fixture::{IMAGE_BYTES, package_with};
use super::model_docx_roundtrip::png;

const OPEN: &str = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body>"#;

fn gif(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = b"GIF89a".to_vec();
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes
}

fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
    bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[0; 12]);
    bytes
}

fn image_block(id: &str, blob: &str, wrap: ImageWrap, width: Option<u32>) -> Block {
    Block::new(
        id,
        BlockKind::Image {
            blob: blob.to_owned(),
            alt: format!("alt {id}"),
            width,
            height: width,
            caption: None,
            wrap,
        },
        "",
    )
}

type Image = (String, String, Option<u32>, Option<u32>, ImageWrap);

fn images(bytes: &[u8]) -> Vec<Image> {
    import_rich(bytes)
        .expect("import")
        .blocks
        .into_iter()
        .filter_map(|block| match block.kind {
            BlockKind::Image {
                blob,
                alt,
                width,
                height,
                wrap,
                ..
            } => Some((blob, alt, width, height, wrap)),
            _ => None,
        })
        .collect()
}

fn media(entries: &[(&[u8], &str)]) -> Media {
    let mut media = Media::new();
    for (bytes, mime) in entries {
        media.insert(
            content_hash(bytes).as_str().to_owned(),
            MediaBlob {
                bytes: bytes.to_vec(),
                mime: (*mime).to_owned(),
            },
        );
    }
    media
}

#[test]
fn every_wrap_round_trips_through_an_anchor() {
    let picture = png(64, 32);
    let hash = content_hash(&picture).as_str().to_owned();
    let wraps = [
        ImageWrap::Inline,
        ImageWrap::Square,
        ImageWrap::TopBottom,
        ImageWrap::Behind,
        ImageWrap::InFront,
    ];
    let blocks = wraps
        .iter()
        .enumerate()
        .map(|(at, wrap)| image_block(&format!("i{at}"), &hash, *wrap, Some(40)))
        .collect();
    let out = materialise_rich_with(
        &blank_docx().expect("blank"),
        &RichText::of(blocks),
        &media(&[(&picture, "image/png")]),
    )
    .expect("materialise");
    let read = images(&out);
    assert_eq!(read.iter().map(|i| i.4).collect::<Vec<_>>(), wraps.to_vec());
    for (blob, alt, width, height, _) in &read {
        assert!(blob.starts_with("docx-media:media/"));
        assert!(alt.starts_with("alt i"));
        assert_eq!((*width, *height), (Some(40), Some(40)));
    }
    let (mime, held) = held_image(&out, &read[0].0, MAX_PART_BYTES).expect("held");
    assert_eq!(mime, "image/png");
    assert_eq!(held, picture);
}

#[test]
fn an_unsized_image_takes_its_pixel_size_from_png_gif_and_jpeg_headers() {
    let (p, g, j) = (png(120, 60), gif(30, 15), jpeg(200, 100));
    let blocks = [&p, &g, &j]
        .iter()
        .enumerate()
        .map(|(at, bytes)| {
            image_block(
                &format!("u{at}"),
                content_hash(bytes).as_str(),
                ImageWrap::Inline,
                None,
            )
        })
        .collect();
    let out = materialise_rich_with(
        &blank_docx().expect("blank"),
        &RichText::of(blocks),
        &media(&[(&p, "image/png"), (&g, "image/gif"), (&j, "image/jpeg")]),
    )
    .expect("materialise");
    let sizes: Vec<(Option<u32>, Option<u32>)> =
        images(&out).into_iter().map(|i| (i.2, i.3)).collect();
    assert_eq!(
        sizes,
        vec![
            (Some(120), Some(60)),
            (Some(30), Some(15)),
            (Some(200), Some(100))
        ]
    );
    let mimes: Vec<String> = images(&out)
        .iter()
        .filter_map(|i| held_image(&out, &i.0, MAX_PART_BYTES).map(|(mime, _)| mime))
        .collect();
    assert_eq!(mimes, vec!["image/png", "image/gif", "image/jpeg"]);
    assert_eq!(
        held_image(&out, content_hash(&g).as_str(), MAX_PART_BYTES).map(|(_, bytes)| bytes),
        Some(g.clone())
    );
}

fn drawing(frame: &str) -> String {
    format!("{OPEN}<w:p><w:r><w:drawing>{frame}</w:drawing></w:r></w:p></w:body></w:document>")
}

fn anchored(attrs: &str, wrap: &str, doc_pr: &str) -> String {
    drawing(&format!(
        "<wp:anchor{attrs}><wp:extent cx=\"952500\" cy=\"476250\"/>{wrap}{doc_pr}<a:graphic><a:graphicData><a:blip r:embed=\"rId9\"/></a:graphicData></a:graphic></wp:anchor>"
    ))
}

fn rels_with_image(bytes: &[u8]) -> Vec<u8> {
    let rels = super::docx_fixture::part(bytes, "word/_rels/document.xml.rels").replace(
        "</Relationships>",
        "<Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image1.png\"/></Relationships>",
    );
    super::pptx_fixture::rebuild(bytes, &[("word/_rels/document.xml.rels", rels)])
}

#[test]
fn anchor_wraps_and_alt_text_are_read_from_word_markup() {
    let cases = [
        ("", "<wp:wrapTight/>", ImageWrap::Square),
        ("", "<wp:wrapThrough/>", ImageWrap::Square),
        ("", "<wp:wrapTopAndBottom/>", ImageWrap::TopBottom),
        (" behindDoc=\"1\"", "<wp:wrapNone/>", ImageWrap::Behind),
        (" behindDoc=\"0\"", "<wp:wrapNone/>", ImageWrap::InFront),
    ];
    for (attrs, wrap, expected) in cases {
        let bytes = rels_with_image(&package_with(&anchored(
            attrs,
            wrap,
            "<wp:docPr id=\"1\" name=\"P\" title=\"Titled\"/>",
        )));
        let read = images(&bytes);
        assert_eq!(read.len(), 1, "{wrap}");
        assert_eq!(read[0].4, expected, "{wrap}");
        assert_eq!(read[0].1, "Titled");
        assert_eq!((read[0].2, read[0].3), (Some(100), Some(50)));
        assert_eq!(read[0].0, "docx-media:media/image1.png");
    }
}

#[test]
fn a_drawing_without_a_frame_or_known_relationship_is_not_an_image() {
    let unknown = package_with(&anchored("", "<wp:wrapNone/>", ""));
    assert!(images(&unknown).is_empty());
    let frameless = rels_with_image(&package_with(&drawing("<wp:other/>")));
    assert!(images(&frameless).is_empty());
}

#[test]
fn held_image_refuses_traversal_unknown_types_and_oversized_parts() {
    let bytes = package_with(&drawing(""));
    assert!(
        held_image(
            &bytes,
            "docx-media:../word/media/image1.png",
            MAX_PART_BYTES
        )
        .is_none()
    );
    assert!(held_image(&bytes, "docx-media:media//image1.png", MAX_PART_BYTES).is_none());
    assert!(held_image(&bytes, "docx-media:media/missing.png", MAX_PART_BYTES).is_none());
    assert!(held_image(&bytes, "not-a-hash", MAX_PART_BYTES).is_none());
    assert!(held_image(b"not a zip", "docx-media:media/image1.png", MAX_PART_BYTES).is_none());
    let (mime, held) =
        held_image(&bytes, "docx-media:media/image1.png", MAX_PART_BYTES).expect("held");
    assert_eq!((mime.as_str(), held.as_slice()), ("image/png", IMAGE_BYTES));
    assert!(held_image(&bytes, "docx-media:media/image1.png", 4).is_none());
    let typeless =
        super::pptx_fixture::rebuild(&bytes, &[("word/media/x.bin", "plain".to_owned())]);
    assert!(held_image(&typeless, "docx-media:media/x.bin", MAX_PART_BYTES).is_none());
}
