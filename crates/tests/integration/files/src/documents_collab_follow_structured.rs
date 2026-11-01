use std::sync::Arc;
use std::time::Duration;

use systemprompt_files::documents::collab::{CollabService, CollabTuning, JoinMode};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::{CellValue, EditOp};

use crate::documents_collab::{Collab, collab, loro};
use crate::documents_service_fixture::{api_author, create, person, trace};

fn following(c: &Collab) -> Arc<CollabService> {
    Arc::new(CollabService::new(
        Arc::clone(&c.documents),
        CollabTuning {
            snapshot_after: Duration::from_secs(3600),
            snapshot_after_updates: 10_000,
            checkpoint_after: Duration::from_secs(3600),
            tick: Duration::from_millis(10),
            evict_after: Duration::from_secs(3600),
        },
        loro(),
    ))
}

async fn joined(
    c: &Collab,
    service: &CollabService,
    owner: &AuthenticatedUser,
    template: &str,
) -> DocumentId {
    let (document, _) = c
        .documents
        .create_from_template(owner, create(template, None), &trace())
        .await
        .expect("create");
    service
        .join(owner, &document.id, JoinMode::Edit, &trace())
        .await
        .expect("join");
    document.id
}

async fn outside(
    c: &Collab,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    ops: Vec<EditOp>,
) -> DocumentRevisionId {
    let (head, _) = c
        .documents
        .read_bytes(owner, document, None, &trace())
        .await
        .expect("read");
    let outcome = c
        .documents
        .commit(
            owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: Some(head.id),
                body: CommitBody::Ops(ops),
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
        .expect("outside commit");
    let CommitOutcome::Committed(landed) = outcome else {
        panic!("{outcome:?}");
    };
    landed.revision.id
}

async fn checkpoint(c: &Collab, document: &DocumentId) -> Option<DocumentRevisionId> {
    c.h.repos
        .edit_sessions
        .find_open(document)
        .await
        .expect("find")
        .expect("session stays open")
        .checkpoint_revision_id
}

async fn followed(
    c: &Collab,
    service: &CollabService,
    document: &DocumentId,
    written: &DocumentRevisionId,
) {
    for _ in 0..200 {
        service.tick().await;
        if checkpoint(c, document).await.as_ref() == Some(written) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the session never followed the outside write");
}

async fn head(c: &Collab, owner: &AuthenticatedUser, document: &DocumentId) -> DocumentRevisionId {
    c.documents
        .read_bytes(owner, document, None, &trace())
        .await
        .expect("read")
        .0
        .id
}

#[tokio::test]
async fn an_idle_workbook_session_follows_an_outside_cell_write() {
    let c = collab().await;
    let service = following(&c);
    let owner = person("follow-sheet");
    let document = joined(&c, &service, &owner, "blank-xlsx").await;
    let written = outside(
        &c,
        &owner,
        &document,
        vec![EditOp::SetCells {
            sheet: "Sheet1".to_owned(),
            range: "A1:B1".to_owned(),
            values: vec![vec![
                CellValue::Text("region".to_owned()),
                CellValue::Number(42.0),
            ]],
            expected_hash: None,
        }],
    )
    .await;

    followed(&c, &service, &document, &written).await;

    assert_eq!(checkpoint(&c, &document).await, Some(written.clone()));
    assert_eq!(head(&c, &owner, &document).await, written);
    service.tick().await;
    assert_eq!(checkpoint(&c, &document).await, Some(written.clone()));
    assert_eq!(head(&c, &owner, &document).await, written);
}

#[tokio::test]
async fn an_idle_deck_session_follows_an_outside_slide() {
    let c = collab().await;
    let service = following(&c);
    let owner = person("follow-deck");
    let document = joined(&c, &service, &owner, "blank-pptx").await;
    let written = outside(
        &c,
        &owner,
        &document,
        vec![EditOp::InsertSlide {
            after: None,
            layout: None,
            title: Some("Followed".to_owned()),
            body: vec!["from outside".to_owned()],
        }],
    )
    .await;

    followed(&c, &service, &document, &written).await;

    assert_eq!(checkpoint(&c, &document).await, Some(written.clone()));
    assert_eq!(head(&c, &owner, &document).await, written);
}
