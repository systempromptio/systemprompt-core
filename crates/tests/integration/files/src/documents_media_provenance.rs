use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, CopyRequest, ImportRequest, ProposeBodyRequest,
    ShareRequest, media_validator,
};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, DriveDeviceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentFormat, DriveCommitMeta, EditOp};
use systemprompt_provider_contracts::documents::content_hash;
use uuid::Uuid;

use crate::documents_media_admit::{uploads_beside, valid_png};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};
use crate::uploads_rig::request;

pub(crate) fn distinct_picture() -> Vec<u8> {
    let seed = Uuid::new_v4().as_u128();
    let width = u32::try_from(seed % 4000).unwrap_or(0) + 1;
    let height = u32::try_from((seed >> 64) % 4000).unwrap_or(0) + 1;
    valid_png(width, height)
}

pub(crate) async fn upload(h: &Harness, who: &AuthenticatedUser, picture: &[u8]) -> String {
    uploads_beside(h)
        .await
        .put_inline(
            &who.id,
            request("picture.png", "image/png", picture),
            picture.to_vec(),
            &trace(),
        )
        .await
        .expect("upload");
    content_hash(picture).as_str().to_owned()
}

pub(crate) fn linking(hash: &str) -> Vec<u8> {
    format!("# Picture\n\n![picture]({hash})\n").into_bytes()
}

pub(crate) async fn markdown(h: &Harness, who: &AuthenticatedUser) -> DocumentId {
    h.service
        .create_from_template(who, create("blank-markdown", None), &trace())
        .await
        .expect("create markdown")
        .0
        .id
}

pub(crate) async fn head(h: &Harness, document: &DocumentId) -> DocumentRevisionId {
    h.repos
        .revisions
        .head(document)
        .await
        .expect("head")
        .expect("a head")
        .id
}

pub(crate) fn commit(
    document: &DocumentId,
    base: DocumentRevisionId,
    body: CommitBody,
) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: Some(base),
        body,
        author: api_author(),
        message: None,
        lock_holder: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    }
}

pub(crate) async fn admitted(h: &Harness, document: &DocumentId, hash: &str) -> bool {
    let hash = systemprompt_identifiers::ContentHash::try_new(hash).expect("hash");
    h.repos
        .media
        .is_admitted(document, &hash)
        .await
        .expect("admitted")
}

pub(crate) fn is_media_refusal(error: &DocumentsError) -> bool {
    matches!(
        error,
        DocumentsError::ContentRefused {
            code: "media_not_admitted",
            ..
        }
    )
}

fn drive_meta(
    document: &DocumentId,
    name: &str,
    base: DocumentRevisionId,
    bytes: &[u8],
) -> DriveCommitMeta {
    DriveCommitMeta {
        document_id: Some(document.clone()),
        path: name.to_owned(),
        base_revision: Some(base),
        content_sha256: content_hash(bytes),
        size_bytes: bytes.len() as u64,
        device_id: DriveDeviceId::new("laptop"),
        message: None,
    }
}

async fn owner_document(h: &Harness, owner: &AuthenticatedUser, hash: &str) -> DocumentId {
    let document = markdown(h, owner).await;
    let base = head(h, &document).await;
    let outcome = h
        .service
        .commit(
            owner,
            commit(&document, base, CommitBody::Bytes(linking(hash))),
            &trace(),
        )
        .await
        .expect("owner commit");
    assert!(
        matches!(outcome, CommitOutcome::Committed(_)),
        "{outcome:?}"
    );
    document
}

#[tokio::test]
async fn a_hash_the_writer_only_knows_is_refused_on_every_commit_path() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let alice = person("provenance-alice");
    let bob = person("provenance-bob");
    let picture = distinct_picture();
    let hash = upload(&h, &alice, &picture).await;
    let theirs = owner_document(&h, &alice, &hash).await;
    assert!(admitted(&h, &theirs, &hash).await);

    let own = markdown(&h, &bob).await;
    let base = head(&h, &own).await;

    let bytes = h
        .service
        .commit(
            &bob,
            commit(&own, base.clone(), CommitBody::Bytes(linking(&hash))),
            &trace(),
        )
        .await
        .expect_err("bytes commit");
    assert!(is_media_refusal(&bytes), "{bytes}");

    let text = String::from_utf8(linking(&hash)).expect("utf8");
    let ops = h
        .service
        .commit(
            &bob,
            commit(
                &own,
                base.clone(),
                CommitBody::Ops(vec![EditOp::SetText { text }]),
            ),
            &trace(),
        )
        .await
        .expect_err("ops commit");
    assert!(is_media_refusal(&ops), "{ops}");

    let name = h.service.get(&bob, &own, &trace()).await.expect("get");
    let drive = h
        .service
        .drive_commit(
            &bob,
            drive_meta(&own, &name.document.name, base.clone(), &linking(&hash)),
            linking(&hash),
            &trace(),
        )
        .await
        .expect_err("drive commit");
    assert!(is_media_refusal(&drive), "{drive}");

    let proposed = h
        .service
        .propose_body(
            &bob,
            ProposeBodyRequest {
                document_id: own.clone(),
                base_revision: base.clone(),
                bytes: linking(&hash),
                author: api_author(),
                message: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect_err("proposal");
    assert!(is_media_refusal(&proposed), "{proposed}");

    let imported = h
        .service
        .import(
            &bob,
            ImportRequest {
                folder: None,
                name: format!("{}.md", unique("import")),
                format: Some(DocumentFormat::Markdown),
                mime_type: None,
                bytes: linking(&hash),
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect_err("import");
    assert!(is_media_refusal(&imported), "{imported}");

    assert_eq!(head(&h, &own).await, base, "no refused write landed");
    assert!(!admitted(&h, &own, &hash).await);
    let served = h
        .service
        .held_image(&bob, &own, &hash, &trace())
        .await
        .expect_err("not served to the writer who only knows the hash");
    assert!(
        matches!(served, DocumentsError::NotFound { .. }),
        "{served}"
    );
}

#[tokio::test]
async fn a_linked_hash_without_an_admission_row_is_not_served_or_exported() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let alice = person("provenance-row");
    let picture = distinct_picture();
    let hash = upload(&h, &alice, &picture).await;
    let document = owner_document(&h, &alice, &hash).await;
    let served = h
        .service
        .held_image(&alice, &document, &hash, &trace())
        .await
        .expect("admitted image");
    assert_eq!(served.bytes, picture);

    let db = systemprompt_test_fixtures::test_db_pool().await;
    sqlx::query("DELETE FROM document_media WHERE document_id = $1::text::uuid")
        .bind(document.as_str())
        .execute(db.write_pool().as_ref())
        .await
        .expect("drop admission");

    let refused = h
        .service
        .held_image(&alice, &document, &hash, &trace())
        .await
        .expect_err("a named hash alone is not served");
    assert!(
        matches!(refused, DocumentsError::NotFound { .. }),
        "{refused}"
    );
    let exported = h
        .service
        .export_markdown(&alice, &document, &trace())
        .await
        .expect("export");
    assert!(
        !exported.markdown.contains("data:image/png"),
        "{}",
        exported.markdown
    );
}

#[tokio::test]
async fn an_owner_upload_is_admitted_and_served_on_every_commit_path() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let alice = person("provenance-paths");

    let picture = distinct_picture();
    let hash = upload(&h, &alice, &picture).await;
    let by_bytes = owner_document(&h, &alice, &hash).await;

    let picture_ops = distinct_picture();
    let hash_ops = upload(&h, &alice, &picture_ops).await;
    let by_ops = markdown(&h, &alice).await;
    let base = head(&h, &by_ops).await;
    let text = String::from_utf8(linking(&hash_ops)).expect("utf8");
    h.service
        .commit(
            &alice,
            commit(
                &by_ops,
                base,
                CommitBody::Ops(vec![EditOp::SetText { text }]),
            ),
            &trace(),
        )
        .await
        .expect("ops commit");

    let picture_drive = distinct_picture();
    let hash_drive = upload(&h, &alice, &picture_drive).await;
    let by_drive = markdown(&h, &alice).await;
    let base = head(&h, &by_drive).await;
    let name = h
        .service
        .get(&alice, &by_drive, &trace())
        .await
        .expect("get");
    h.service
        .drive_commit(
            &alice,
            drive_meta(&by_drive, &name.document.name, base, &linking(&hash_drive)),
            linking(&hash_drive),
            &trace(),
        )
        .await
        .expect("drive commit");

    let picture_import = distinct_picture();
    let hash_import = upload(&h, &alice, &picture_import).await;
    let (by_import, _) = h
        .service
        .import(
            &alice,
            ImportRequest {
                folder: None,
                name: format!("{}.md", unique("import")),
                format: Some(DocumentFormat::Markdown),
                mime_type: None,
                bytes: linking(&hash_import),
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("import");

    for (document, hash, picture) in [
        (&by_bytes, &hash, &picture),
        (&by_ops, &hash_ops, &picture_ops),
        (&by_drive, &hash_drive, &picture_drive),
        (&by_import.id, &hash_import, &picture_import),
    ] {
        assert!(admitted(&h, document, hash).await, "{document}");
        let served = h
            .service
            .held_image(&alice, document, hash, &trace())
            .await
            .expect("served");
        assert_eq!(&served.bytes, picture);
    }

    let pdf = h
        .service
        .render_pdf(&alice, &by_bytes, None, &trace())
        .await
        .expect("pdf");
    assert!(!pdf.bytes.is_empty());
}

fn view_grant(document: &DocumentId, who: &AuthenticatedUser) -> ShareRequest {
    ShareRequest {
        target: GrantTarget::Document(document.clone()),
        subject_kind: "user".to_owned(),
        subject_value: who.id.as_str().to_owned(),
        subject_display: None,
        level: AccessLevel::View,
        valid_until: None,
        message: None,
        via_agent: None,
    }
}

#[tokio::test]
async fn a_copy_carries_admissions_and_a_revoked_share_ends_image_access() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let alice = person("provenance-share-owner");
    let bob = person("provenance-share-viewer");
    let picture = distinct_picture();
    let hash = upload(&h, &alice, &picture).await;
    let document = owner_document(&h, &alice, &hash).await;
    let shared = h
        .service
        .share(&alice, view_grant(&document, &bob), &trace())
        .await
        .expect("share");

    let seen = h
        .service
        .held_image(&bob, &document, &hash, &trace())
        .await
        .expect("viewer sees the image");
    let validator = media_validator(&document, &seen);
    assert_ne!(
        validator.as_str(),
        hash,
        "the validator is not the blob hash"
    );

    let (copy, _) = h
        .service
        .copy(
            &bob,
            CopyRequest {
                source: document.clone(),
                name: None,
                folder: None,
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("copy");
    assert!(admitted(&h, &copy.id, &hash).await);
    let copied = h
        .service
        .held_image(&bob, &copy.id, &hash, &trace())
        .await
        .expect("the copy serves the image it carried");
    assert_ne!(
        media_validator(&copy.id, &copied),
        validator,
        "a validator does not carry across documents"
    );

    h.service
        .revoke(
            &alice,
            &GrantTarget::Document(document.clone()),
            shared.id,
            &trace(),
        )
        .await
        .expect("revoke");
    let revoked = h
        .service
        .held_image(&bob, &document, &hash, &trace())
        .await
        .expect_err("a revoked viewer is refused before any validator check");
    assert!(
        matches!(
            revoked,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{revoked}"
    );
}

#[tokio::test]
async fn landing_refuses_an_admitted_image_whose_blob_is_gone() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let alice = person("provenance-collected");
    let document = markdown(&h, &alice).await;
    let gone = content_hash(&distinct_picture()).as_str().to_owned();
    let db = systemprompt_test_fixtures::test_db_pool().await;
    let mut conn = db.write_pool().acquire().await.expect("acquire");
    let missing = systemprompt_files::documents::repository::DocumentMediaRepository::admit_in(
        &mut conn,
        &document,
        std::slice::from_ref(&gone),
        &alice.id,
    )
    .await
    .expect("admit");
    assert_eq!(missing, vec![gone.clone()]);
    assert!(!admitted(&h, &document, &gone).await);
}
