use std::io::Write as _;
use std::sync::Arc;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::model::layout::ImageWrap;
use systemprompt_files::documents::model::{Block, BlockKind, Media, MediaBlob, RichText};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::content_hash;
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::FileStorage;
use tempfile::TempDir;
use uuid::Uuid;

use crate::documents_media_read::png;
use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, create, harness, harness_configured, person, trace,
};

const CAP: usize = 64;

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

fn decodable_png(side: u32) -> Vec<u8> {
    let mut raw = Vec::new();
    let mut noise = Vec::new();
    while noise.len() < (side * side * 3) as usize {
        noise.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    let row = (side * 3) as usize;
    for line in noise.chunks(row).take(side as usize) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw).expect("deflate");
    let mut ihdr = side.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&side.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(chunk(b"IHDR", &ihdr));
    bytes.extend(chunk(b"IDAT", &encoder.finish().expect("finish")));
    bytes.extend(chunk(b"IEND", &[]));
    bytes
}

fn store(h: &Harness) -> BlobStore {
    let storage: Arc<dyn FileStorage> =
        Arc::new(LocalFileStorage::new(h.storage.path().to_path_buf()));
    BlobStore::new(storage, Arc::clone(&h.repos.blobs))
}

async fn with_pictures(h: &Harness, owner: &AuthenticatedUser, pictures: &[Vec<u8>]) -> DocumentId {
    let mut blocks = Vec::new();
    let mut media = Media::new();
    for (index, picture) in pictures.iter().enumerate() {
        store(h).put(picture).await.expect("put picture");
        let hash = content_hash(picture).as_str().to_owned();
        blocks.push(Block::new(
            &format!("i{index}"),
            BlockKind::Image {
                blob: hash.clone(),
                alt: format!("picture {index}"),
                width: None,
                height: None,
                caption: None,
                wrap: ImageWrap::Inline,
            },
            "",
        ));
        media.insert(
            hash,
            MediaBlob {
                bytes: picture.clone(),
                mime: "image/png".to_owned(),
            },
        );
    }
    let mut request = create("blank-markdown", None);
    request.content = Some(RichText::of(blocks));
    request.media = media;
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create with pictures")
        .0
        .id
}

fn image_objects(pdf: &[u8]) -> usize {
    pdf.windows(b"/Subtype /Image".len())
        .filter(|window| *window == b"/Subtype /Image")
        .count()
}

#[tokio::test]
async fn markdown_export_embeds_at_most_the_cap_and_names_the_overflow() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("export-cap-md");
    let seed = u32::try_from(Uuid::new_v4().as_u128() % 100_000).unwrap_or(0) + 10;
    let pictures: Vec<Vec<u8>> = (0..=CAP)
        .map(|index| png(seed, u32::try_from(index).expect("index") + 1))
        .collect();
    let document = with_pictures(&h, &owner, &pictures).await;
    let exported = h
        .service
        .export_markdown(&owner, &document, &trace())
        .await
        .expect("export");
    assert_eq!(exported.markdown.matches("data:image/png").count(), CAP);
    assert!(
        exported
            .left_out
            .iter()
            .any(|note| note.starts_with("1 image(s)")),
        "{:?}",
        exported.left_out
    );
}

#[tokio::test]
async fn pdf_export_follows_at_most_the_cap() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("export-cap-pdf");
    let pictures: Vec<Vec<u8>> = (0..=CAP).map(|_| decodable_png(2)).collect();
    let document = with_pictures(&h, &owner, &pictures).await;
    let pdf = h
        .service
        .render_pdf(&owner, &document, None, &trace())
        .await
        .expect("pdf");
    assert_eq!(image_objects(&pdf.bytes), CAP);
}

#[tokio::test]
async fn pdf_export_stops_reading_images_once_the_byte_budget_is_spent() {
    let pictures: Vec<Vec<u8>> = (0..3).map(|_| decodable_png(16)).collect();
    let largest = pictures.iter().map(Vec::len).max().expect("pictures");
    let config = DocumentsConfig {
        max_upload_bytes: u64::try_from(largest * 2 + 10).expect("budget"),
        ..DocumentsConfig::default()
    };
    let h = harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        config,
    )
    .await;
    let owner = person("export-budget-pdf");
    let single = with_pictures(&h, &owner, &pictures[..1]).await;
    let one = h
        .service
        .render_pdf(&owner, &single, None, &trace())
        .await
        .expect("pdf");
    assert_eq!(
        image_objects(&one.bytes),
        1,
        "a picture within budget is embedded"
    );

    let document = with_pictures(&h, &owner, &pictures).await;
    let pdf = h
        .service
        .render_pdf(&owner, &document, None, &trace())
        .await
        .expect("pdf");
    assert_eq!(image_objects(&pdf.bytes), 2);
}
