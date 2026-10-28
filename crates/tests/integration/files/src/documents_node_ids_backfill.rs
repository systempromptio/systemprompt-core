//! Legacy revisions without node ids: reads refuse instead of minting, and
//! the backfill job mints them once, carrying ids parent to child.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::jobs::DocumentNodeIdsBackfillJob;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
};

async fn memo(h: &Harness, owner: &AuthenticatedUser) -> (DocumentId, DocumentRevisionId) {
    let mut request = create("blank-docx", None);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: "alpha\n\nbeta".to_owned(),
    }];
    let (document, revision) = h
        .service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create memo");
    (document.id, revision.id)
}

async fn id_rows(revision: &DocumentRevisionId) -> i64 {
    let pool = test_db_pool().await.pool();
    sqlx::query_scalar("SELECT COUNT(*) FROM document_node_ids WHERE revision_id = $1::uuid")
        .bind(revision.as_str())
        .fetch_one(&*pool)
        .await
        .expect("count node ids")
}

async fn forget_ids(revision: &DocumentRevisionId) {
    let pool = test_db_pool().await.pool();
    sqlx::query("DELETE FROM document_node_ids WHERE revision_id = $1::uuid")
        .bind(revision.as_str())
        .execute(&*pool)
        .await
        .expect("forget node ids");
}

#[tokio::test]
async fn a_legacy_revision_refuses_reads_without_writing_until_backfilled() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("legacy");
    let (document, revision) = memo(&h, &owner).await;
    let (_, before) = h
        .service
        .project(&owner, &document, Some(&revision), &trace())
        .await
        .expect("project");
    let minted = id_rows(&revision).await;
    assert!(minted > 0);
    forget_ids(&revision).await;

    for _ in 0..2 {
        let refused = h
            .service
            .project(&owner, &document, Some(&revision), &trace())
            .await
            .expect_err("a revision without ids refuses");
        assert!(
            matches!(
                &refused,
                DocumentsError::NotFound {
                    what: "revision",
                    ..
                }
            ),
            "{refused}"
        );
        assert_eq!(id_rows(&revision).await, 0, "a read never mints");
    }

    let report = DocumentNodeIdsBackfillJob::backfill(&h.service)
        .await
        .expect("backfill");
    assert!(report.minted >= 1, "{report:?}");
    assert_eq!(id_rows(&revision).await, minted);
    let (_, after) = h
        .service
        .project(&owner, &document, Some(&revision), &trace())
        .await
        .expect("project after backfill");
    assert_eq!(
        after.nodes().map(|n| n.text.clone()).collect::<Vec<_>>(),
        before.nodes().map(|n| n.text.clone()).collect::<Vec<_>>()
    );
}
