//! `ReadPoolLag`: a document service whose read pool is a replica frozen
//! behind the primary. Every read a commit depends on must come from the
//! write pool, so a commit built on a revision that landed after the replica
//! fell behind still lands, while plain reads on the replica keep trailing.

use std::sync::Arc;

use sqlx::Executor;
use sqlx::postgres::PgPoolOptions;
use systemprompt_database::{Database, DbPool};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, HistoryPage, LandedCommit,
};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{EditOp, Projection};
use systemprompt_test_fixtures::{test_database_url, test_db_pool};
use tempfile::TempDir;

use crate::documents_proposals::{memo, outline, rewrite};
use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, api_author, harness_on_db, person, trace,
};

struct ReadPoolLag {
    harness: Harness,
    read: Arc<sqlx::PgPool>,
}

impl ReadPoolLag {
    async fn start() -> Self {
        let primary = test_db_pool().await;
        let read = PgPoolOptions::new()
            .max_connections(1)
            .connect(&test_database_url())
            .await
            .expect("lagging read pool");
        let read = Arc::new(read);
        let db: DbPool = Arc::new(Database::from_pools(
            Arc::clone(&read),
            Some(primary.write_pool()),
        ));
        let harness = harness_on_db(
            db,
            allow(),
            GroupSource::new(&[]),
            Arc::new(TempDir::new().expect("tempdir")),
            RoleDirectory::default(),
            DocumentsConfig::default(),
        );
        Self { harness, read }
    }

    async fn freeze(&self) {
        let mut replica = self.read.acquire().await.expect("the replica connection");
        replica
            .execute("BEGIN ISOLATION LEVEL REPEATABLE READ")
            .await
            .expect("open the replica transaction");
        replica
            .execute("SELECT 1")
            .await
            .expect("freeze the replica snapshot");
    }

    async fn replica_revisions(&self, document: &DocumentId) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM document_revisions WHERE document_id = $1::uuid",
        )
        .bind(document.as_str())
        .fetch_one(&*self.read)
        .await
        .expect("count on the replica")
    }
}

fn request(document: &DocumentId, ops: Vec<EditOp>) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(ops),
        author: api_author(),
        message: None,
        lock_holder: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    }
}

async fn land(
    lag: &ReadPoolLag,
    who: &AuthenticatedUser,
    document: &DocumentId,
    projection: &Projection,
    from: &str,
    to: &str,
) -> LandedCommit {
    let outcome = lag
        .harness
        .service
        .commit(
            who,
            request(document, vec![rewrite(projection, from, to)]),
            &trace(),
        )
        .await
        .expect("commit");
    match outcome {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => *landed,
        CommitOutcome::Conflict { head, .. } => panic!("conflict at {}", head.id),
    }
}

#[tokio::test]
async fn a_commit_on_a_revision_the_replica_has_not_seen_still_lands() {
    let lag = ReadPoolLag::start().await;
    let owner = person("lag-owner");
    let (document, first) = memo(&lag.harness, &owner, None).await;
    let seen_by_replica = outline(&lag.harness, &owner, &document, None).await;
    lag.freeze().await;
    assert_eq!(lag.replica_revisions(&document).await, 1);

    let second = land(&lag, &owner, &document, &seen_by_replica, "alpha", "ALPHA").await;
    assert_eq!(second.revision.parent_revision_id.as_ref(), Some(&first));
    assert!(second.revision.landed_at.is_some());
    assert_eq!(lag.replica_revisions(&document).await, 1);

    let third = land(&lag, &owner, &document, &second.projection, "beta", "BETA").await;
    assert_eq!(
        third.revision.parent_revision_id.as_ref(),
        Some(&second.revision.id)
    );
    assert!(third.revision.landed_at.is_some());

    let head = lag
        .harness
        .repos
        .revisions
        .head(&document)
        .await
        .expect("head")
        .expect("a head");
    assert_eq!(head.id, third.revision.id);
    let history = lag
        .harness
        .service
        .history(
            &owner,
            &document,
            HistoryPage {
                limit: 10,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    assert_eq!(
        history.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        vec![first]
    );
    assert_eq!(lag.replica_revisions(&document).await, 1);
}
