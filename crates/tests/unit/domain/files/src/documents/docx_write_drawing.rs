use systemprompt_files::documents::codecs::docx::{blank_docx, import_rich, materialise_rich_with};
use systemprompt_files::documents::model::layout::ImageWrap;
use systemprompt_files::documents::model::{Block, BlockKind, Media, MediaBlob, RichText};
use systemprompt_provider_contracts::documents::content_hash;

use super::docx_fixture::part;
use super::model_docx_roundtrip::png;

fn picture(blob: &str, width: Option<u32>, height: Option<u32>, caption: Option<&str>) -> Block {
    Block::new(
        "img",
        BlockKind::Image {
            blob: blob.to_owned(),
            alt: "a \"quoted\" <alt>".to_owned(),
            width,
            height,
            caption: caption.map(str::to_owned),
            wrap: ImageWrap::Inline,
        },
        "",
    )
}

fn media_of(bytes: &[u8]) -> (String, Media) {
    let hash = content_hash(bytes).as_str().to_owned();
    let mut media = Media::new();
    media.insert(
        hash.clone(),
        MediaBlob {
            bytes: bytes.to_vec(),
            mime: "image/png".to_owned(),
        },
    );
    (hash, media)
}

fn write(blocks: Vec<Block>, media: &Media) -> Result<Vec<u8>, String> {
    materialise_rich_with(&blank_docx().expect("blank"), &RichText::of(blocks), media)
        .map_err(|error| error.to_string())
}

fn sizes(bytes: &[u8]) -> Vec<(Option<u32>, Option<u32>)> {
    import_rich(bytes)
        .expect("import")
        .blocks
        .into_iter()
        .filter_map(|block| match block.kind {
            BlockKind::Image { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .collect()
}

#[test]
fn one_given_side_scales_the_other_from_the_natural_size() {
    let bytes = png(200, 100);
    let (hash, media) = media_of(&bytes);
    let out = write(
        vec![
            picture(&hash, Some(100), None, None),
            picture(&hash, None, Some(25), None),
        ],
        &media,
    )
    .expect("write");
    assert_eq!(
        sizes(&out),
        vec![(Some(100), Some(50)), (Some(50), Some(25))]
    );
    let document = part(&out, "word/document.xml");
    assert!(document.contains("descr=\"a &quot;quoted&quot; &lt;alt&gt;\""));
    assert_eq!(
        part(&out, "[Content_Types].xml")
            .matches("Extension=\"png\"")
            .count(),
        1
    );
}

#[test]
fn a_wide_unsized_picture_is_capped_to_the_page_and_a_caption_follows_it() {
    let bytes = png(1248, 200);
    let (hash, media) = media_of(&bytes);
    let out = write(
        vec![picture(&hash, None, None, Some("Figure & one"))],
        &media,
    )
    .expect("write");
    assert_eq!(sizes(&out), vec![(Some(624), Some(100))]);
    let document = part(&out, "word/document.xml");
    assert!(document.contains("<w:pStyle w:val=\"Caption\"/>"));
    assert!(document.contains("Figure &amp; one"));
}

#[test]
fn an_existing_media_part_is_reused_by_its_docx_media_reference() {
    let bytes = png(40, 20);
    let (hash, media) = media_of(&bytes);
    let first = write(vec![picture(&hash, None, None, None)], &media).expect("write");
    let model = import_rich(&first).expect("import");
    let again = materialise_rich_with(&first, &model, &Media::new()).expect("again");
    assert_eq!(sizes(&again), vec![(Some(40), Some(20))]);
    let mut copy = model.clone();
    copy.blocks.push(copy.blocks[0].clone());
    let doubled = materialise_rich_with(&first, &copy, &Media::new()).expect("doubled");
    assert_eq!(sizes(&doubled).len(), 2);
}

#[test]
fn unresolvable_or_foreign_pictures_refuse_the_write() {
    let unresolved =
        write(vec![picture("deadbeef", None, None, None)], &Media::new()).expect_err("unresolved");
    assert!(unresolved.contains("was not resolved"), "{unresolved}");

    let escaped = write(
        vec![picture("docx-media:../secret.png", None, None, None)],
        &Media::new(),
    )
    .expect_err("escape");
    assert!(
        escaped.contains("does not name a picture part"),
        "{escaped}"
    );

    let text = b"plain text, not a picture".to_vec();
    let (hash, media) = media_of(&text);
    let foreign = write(vec![picture(&hash, None, None, None)], &media).expect_err("not an image");
    assert!(foreign.contains("is not a PNG, JPEG or GIF"), "{foreign}");
}
