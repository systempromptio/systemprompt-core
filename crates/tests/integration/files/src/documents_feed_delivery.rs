use std::time::Duration;

use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitRequest, ShareRequest, parse_drive_cursor,
};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DriveChange, DriveChangeKind, EditOp};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};

async fn document(h: &Harness, who: &AuthenticatedUser) -> (DocumentId, String) {
    let mut request = create("blank-text", None);
    request.name = unique("doc");
    request.ops = vec![EditOp::SetText {
        text: "shared\n".to_owned(),
    }];
    let (document, _) = h
        .service
        .create_from_template(who, request, &trace())
        .await
        .expect("create");
    (document.id, document.name)
}

async fn grant(h: &Harness, owner: &AuthenticatedUser, to: &AuthenticatedUser, doc: &DocumentId) {
    h.service
        .share(
            owner,
            ShareRequest {
                target: GrantTarget::Document(doc.clone()),
                subject_kind: "user".to_owned(),
                subject_value: to.id.as_str().to_owned(),
                subject_display: None,
                level: AccessLevel::View,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share");
}

async fn manifest(h: &Harness, who: &AuthenticatedUser) -> String {
    h.service
        .drive_manifest(who, &trace())
        .await
        .expect("manifest")
        .cursor
}

async fn drain(
    h: &Harness,
    who: &AuthenticatedUser,
    start: &str,
    document: &DocumentId,
) -> Vec<(DriveChangeKind, String)> {
    let mut cursor = parse_drive_cursor(start).expect("cursor parses");
    let mut seen: Vec<DriveChange> = Vec::new();
    for _ in 0..200 {
        let page = h
            .service
            .drive_changes(who, Some(cursor), 2, &trace())
            .await
            .expect("changes");
        seen.extend(
            page.changes
                .into_iter()
                .filter(|change| &change.document_id == document),
        );
        let next = parse_drive_cursor(&page.next_cursor).expect("next cursor");
        if next == cursor {
            break;
        }
        cursor = next;
    }
    seen.into_iter().map(|c| (c.kind, c.path)).collect()
}

async fn settled(
    h: &Harness,
    who: &AuthenticatedUser,
    start: &str,
    document: &DocumentId,
) -> Vec<(DriveChangeKind, String)> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut seen = drain(h, who, start, document).await;
    while seen.is_empty() && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
        seen = drain(h, who, start, document).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let again = drain(h, who, start, document).await;
    assert_eq!(seen, again, "a re-read from the same cursor is stable");
    again
}

#[tokio::test]
async fn a_grantee_whose_cursor_predates_the_document_gets_one_created() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("early-owner");
    let grantee = person("early-grantee");
    let start = manifest(&h, &grantee).await;
    let (doc, name) = document(&h, &owner).await;
    grant(&h, &owner, &grantee, &doc).await;

    assert_eq!(
        settled(&h, &grantee, &start, &doc).await,
        vec![(DriveChangeKind::Created, name)]
    );
}

#[tokio::test]
async fn a_concurrent_revision_writer_does_not_double_deliver_a_shared_document() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("held-owner");
    let grantee = person("held-grantee");
    let db = test_db_pool().await;
    let mut blocker = db.write_pool().begin().await.expect("begin blocker");
    sqlx::query("SELECT txid_current()")
        .execute(&mut *blocker)
        .await
        .expect("assign xid");
    sqlx::query("LOCK TABLE document_revisions IN ROW EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .expect("hold row exclusive");

    let (doc, name) = document(&h, &owner).await;
    let start = manifest(&h, &grantee).await;
    blocker.commit().await.expect("release blocker");
    grant(&h, &owner, &grantee, &doc).await;

    assert_eq!(
        settled(&h, &grantee, &start, &doc).await,
        vec![(DriveChangeKind::Created, name)]
    );
}

#[tokio::test]
async fn an_update_after_the_grant_is_still_delivered() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("after-owner");
    let grantee = person("after-grantee");
    let start = manifest(&h, &grantee).await;
    let (doc, name) = document(&h, &owner).await;
    grant(&h, &owner, &grantee, &doc).await;
    let base = h
        .service
        .get(&owner, &doc, &trace())
        .await
        .expect("get")
        .head
        .map(|head| head.id);
    h.service
        .commit(
            &owner,
            CommitRequest {
                document_id: doc.clone(),
                base_revision: base,
                body: CommitBody::Ops(vec![EditOp::SetText {
                    text: "edited\n".to_owned(),
                }]),
                author: api_author(),
                message: None,
                lock_holder: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("edit");

    assert_eq!(
        settled(&h, &grantee, &start, &doc).await,
        vec![
            (DriveChangeKind::Created, name.clone()),
            (DriveChangeKind::Updated, name)
        ]
    );
}
