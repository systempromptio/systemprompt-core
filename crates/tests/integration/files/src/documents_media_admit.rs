use std::sync::Arc;

use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::repository::DocumentBlobRepository;
use systemprompt_files::documents::service::refuse_unheld_media;
use systemprompt_files::uploads::{DocumentUploadRepository, UploadService, UploadServiceParts};
use systemprompt_files::{DocumentsConfig, UploadLimits};
use systemprompt_identifiers::UserId;
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{BackgroundTasks, FileStorage};

use crate::documents_service_fixture::{GroupSource, Harness, allow, harness};
use crate::uploads_rig::{hash_of, owner, request, trace};

async fn uploads_beside(h: &Harness) -> UploadService {
    let db = test_db_pool().await;
    let storage: Arc<dyn FileStorage> =
        Arc::new(LocalFileStorage::new(h.storage.path().to_path_buf()));
    let blobs = Arc::new(BlobStore::new(
        Arc::clone(&storage),
        Arc::new(DocumentBlobRepository::new(&db)),
    ));
    UploadService::new(UploadServiceParts {
        storage,
        uploads: Arc::new(DocumentUploadRepository::new(&db)),
        blobs,
        max_upload_bytes: DocumentsConfig::default().max_upload_bytes,
        limits: UploadLimits::default(),
        tasks: BackgroundTasks::new(),
    })
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(data.len())
        .expect("len")
        .to_be_bytes()
        .to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&[0; 4]);
    out
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(chunk(b"IHDR", &ihdr));
    bytes.extend(chunk(b"IDAT", &[0x78, 0x9c, 0x63, 0, 0, 0, 1, 0, 1]));
    bytes.extend(chunk(b"IEND", &[]));
    bytes
}

#[test]
fn only_references_the_document_already_holds_survive_without_an_upload() {
    let held = vec!["held-a".to_owned()];
    let admitted = refuse_unheld_media(vec!["held-a".to_owned(), "fresh-b".to_owned()], &held);
    assert!(admitted.media.is_empty());
    assert_eq!(admitted.refused, vec!["fresh-b".to_owned()]);
}

#[tokio::test]
async fn an_image_is_admitted_only_from_an_upload_its_owner_completed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let uploads = uploads_beside(&h).await;
    let uploader = owner();
    let picture = png(4, 3);
    let hash = hash_of(&picture);
    uploads
        .put_inline(
            &uploader,
            request("picture.png", "image/png", &picture),
            picture.clone(),
            &trace(),
        )
        .await
        .expect("upload");

    let owned = uploader.clone();
    let owners = move |_: &str| vec![UserId::new("someone-else"), owned.clone()];
    let refs = vec![
        hash.as_str().to_owned(),
        "not-a-content-hash".to_owned(),
        "held-already".to_owned(),
    ];
    let admitted = h
        .service
        .admit_media(&uploads, &owners, refs, &["held-already".to_owned()])
        .await;
    assert_eq!(admitted.refused, vec!["not-a-content-hash".to_owned()]);
    let blob = admitted.media.get(hash.as_str()).expect("admitted");
    assert_eq!(blob.bytes, picture);
    assert!(!admitted.media.contains_key("held-already"));

    let strangers = |_: &str| vec![UserId::new("never-uploaded")];
    let refused = h
        .service
        .admit_media(&uploads, &strangers, vec![hash.as_str().to_owned()], &[])
        .await;
    assert!(refused.media.is_empty());
    assert_eq!(refused.refused, vec![hash.as_str().to_owned()]);

    let nobody = |_: &str| Vec::new();
    let unowned = h
        .service
        .admit_media(&uploads, &nobody, vec![hash.as_str().to_owned()], &[])
        .await;
    assert_eq!(unowned.refused, vec![hash.as_str().to_owned()]);
}
