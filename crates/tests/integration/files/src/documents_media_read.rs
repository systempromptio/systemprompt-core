use std::sync::Arc;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::codecs::docx::import_rich;
use systemprompt_files::documents::model::layout::ImageWrap;
use systemprompt_files::documents::model::{Block, BlockKind, Media, MediaBlob, RichText};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::content_hash;
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::FileStorage;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, deny, harness, person, trace,
};

pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

fn image(id: &str, blob: &str, sized: bool) -> Block {
    Block::new(
        id,
        BlockKind::Image {
            blob: blob.to_owned(),
            alt: format!("alt {id}"),
            width: sized.then_some(40),
            height: sized.then_some(20),
            caption: None,
            wrap: ImageWrap::Inline,
        },
        "",
    )
}

fn media_of(picture: &[u8]) -> Media {
    let mut media = Media::new();
    media.insert(
        content_hash(picture).as_str().to_owned(),
        MediaBlob {
            bytes: picture.to_vec(),
            mime: "image/png".to_owned(),
        },
    );
    media
}

async fn with_image(
    h: &Harness,
    owner: &AuthenticatedUser,
    template: &str,
    picture: &[u8],
) -> DocumentId {
    let hash = content_hash(picture).as_str().to_owned();
    let mut request = create(template, None);
    request.content = Some(RichText::of(vec![
        Block::new("p1", BlockKind::Paragraph, "Caption above"),
        image("i1", &hash, template == "blank-docx"),
    ]));
    request.media = media_of(picture);
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create with image")
        .0
        .id
}

fn store(h: &Harness) -> BlobStore {
    let storage: Arc<dyn FileStorage> =
        Arc::new(LocalFileStorage::new(h.storage.path().to_path_buf()));
    BlobStore::new(storage, Arc::clone(&h.repos.blobs))
}

fn is_missing_image(error: &DocumentsError) -> bool {
    matches!(error, DocumentsError::NotFound { what: "image", .. })
}

#[tokio::test]
async fn a_docx_serves_the_picture_its_head_embeds_and_nothing_else() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("media-docx");
    let picture = png(40, 20);
    let document = with_image(&h, &owner, "blank-docx", &picture).await;
    let (_, bytes) = h
        .service
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("head");
    let reference = import_rich(&bytes)
        .expect("import")
        .blocks
        .into_iter()
        .find_map(|block| match block.kind {
            BlockKind::Image { blob, .. } => Some(blob),
            _ => None,
        })
        .expect("an embedded image");
    assert!(reference.starts_with("docx-media:media/"));

    let served = h
        .service
        .held_image(&owner, &document, &reference, &trace())
        .await
        .expect("held image");
    assert_eq!(served.mime, "image/png");
    assert_eq!(served.bytes, picture);

    for absent in [
        "docx-media:media/absent.png",
        "docx-media:../word/document.xml",
        "word/document.xml",
    ] {
        let error = h
            .service
            .held_image(&owner, &document, absent, &trace())
            .await
            .expect_err("not held");
        assert!(is_missing_image(&error), "{absent}: {error}");
    }
}

#[tokio::test]
async fn a_markdown_head_serves_only_the_uploaded_blob_it_links() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("media-markdown");
    let picture = png(12, 8);
    let hash = content_hash(&picture).as_str().to_owned();
    store(&h).put(&picture).await.expect("put blob");
    let document = with_image(&h, &owner, "blank-markdown", &picture).await;

    let served = h
        .service
        .held_image(&owner, &document, &hash, &trace())
        .await
        .expect("linked blob");
    assert_eq!(served.mime, "image/png");
    assert_eq!(served.bytes, picture);
    assert_eq!(
        h.service
            .media_bytes(&content_hash(&picture))
            .await
            .expect("media bytes"),
        picture
    );

    let other = png(99, 99);
    store(&h).put(&other).await.expect("put unlinked");
    let unlinked = h
        .service
        .held_image(&owner, &document, content_hash(&other).as_str(), &trace())
        .await
        .expect_err("an uploaded but unlinked blob is not served");
    assert!(is_missing_image(&unlinked), "{unlinked}");
}

#[tokio::test]
async fn a_linked_blob_that_is_absent_or_not_an_image_is_not_served() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("media-missing");
    let picture = png(5, 5);
    let document = with_image(&h, &owner, "blank-markdown", &picture).await;
    let hash = content_hash(&picture).as_str().to_owned();
    let absent = h
        .service
        .held_image(&owner, &document, &hash, &trace())
        .await
        .expect_err("the blob was never stored");
    assert!(
        matches!(absent, DocumentsError::Blob(_)) || is_missing_image(&absent),
        "{absent}"
    );

    let text = b"plain words, not a picture".to_vec();
    let text_hash = content_hash(&text).as_str().to_owned();
    store(&h).put(&text).await.expect("put text blob");
    let mut request = create("blank-markdown", None);
    request.content = Some(RichText::of(vec![image("i1", &text_hash, false)]));
    request.media = {
        let mut media = Media::new();
        media.insert(
            text_hash.clone(),
            MediaBlob {
                bytes: text.clone(),
                mime: "image/png".to_owned(),
            },
        );
        media
    };
    let linked = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create")
        .0
        .id;
    let unsniffed = h
        .service
        .held_image(&owner, &linked, &text_hash, &trace())
        .await
        .expect_err("bytes that are no image format");
    assert!(is_missing_image(&unsniffed), "{unsniffed}");
}

#[tokio::test]
async fn slide_formats_look_only_in_their_media_folder_and_viewers_need_access() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("media-slides");
    for template in ["blank-pptx", "blank-odp"] {
        let document = h
            .service
            .create_from_template(&owner, create(template, None), &trace())
            .await
            .expect("create slides")
            .0
            .id;
        for reference in ["image1.png", "../content.xml", "ppt/slides/slide1.xml"] {
            let error = h
                .service
                .held_image(&owner, &document, reference, &trace())
                .await
                .expect_err("no picture held");
            assert!(is_missing_image(&error), "{template} {reference}: {error}");
        }
    }

    let document = with_image(&h, &owner, "blank-docx", &png(3, 3)).await;
    let denied = harness(deny(), GroupSource::new(&[])).await;
    let error = denied
        .service
        .held_image(
            &person("media-stranger"),
            &document,
            "docx-media:media/x.png",
            &trace(),
        )
        .await
        .expect_err("stranger");
    assert!(
        matches!(
            error,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{error}"
    );
}
