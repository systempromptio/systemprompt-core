use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::jobs::{
    DocumentAgentReadRetentionJob, DocumentBlobGcJob, DocumentCollabReaperJob,
    DocumentCollabUpdateSweepJob, DocumentMediaBackfillJob, DocumentNodeIdsBackfillJob,
    DocumentPresenceSweepJob,
};
use systemprompt_files::uploads::DocumentUploadExpiryJob;
use systemprompt_files::{DocumentsConfig, FilesConfig, FilesConfigYaml};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::documents::EditOp;
use systemprompt_provider_contracts::{Dependencies, Job, JobContext, JobScope};
use systemprompt_test_fixtures::{fixture_actor, test_db_pool};
use uuid::Uuid;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
};

fn files_config(enabled: bool) -> Arc<FilesConfig> {
    let yaml = FilesConfigYaml {
        documents: DocumentsConfig {
            enabled,
            ..DocumentsConfig::default()
        },
        ..FilesConfigYaml::default()
    };
    Arc::new(FilesConfig::new(PathBuf::from("/tmp"), yaml))
}

fn context(db: DbPool, enabled: bool, service: Option<&DocumentService>) -> JobContext {
    let mut deps = Dependencies::new().with(db).with(files_config(enabled));
    if let Some(service) = service {
        deps.insert(Arc::new(service.clone()));
    }
    JobContext::new(fixture_actor(), deps)
}

async fn memo(h: &Harness) -> (DocumentId, DocumentRevisionId) {
    let owner = person("jobs");
    let mut request = create("blank-docx", None);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: "alpha\n\nbeta".to_owned(),
    }];
    let (document, revision) = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create memo");
    (document.id, revision.id)
}

async fn count(sql: &'static str, key: &str) -> i64 {
    let pool = test_db_pool().await.pool();
    sqlx::query_scalar(sql)
        .bind(key.to_owned())
        .fetch_one(&*pool)
        .await
        .expect("count")
}

async fn exec(sql: &'static str, key: &str) {
    let pool = test_db_pool().await.pool();
    sqlx::query(sql)
        .bind(key.to_owned())
        .execute(&*pool)
        .await
        .expect("exec");
}

#[tokio::test]
async fn every_document_job_is_a_cluster_noop_while_the_store_is_disabled() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let ctx = context(test_db_pool().await, false, Some(&h.service));
    let jobs: Vec<Box<dyn Job>> = vec![
        Box::new(DocumentAgentReadRetentionJob),
        Box::new(DocumentBlobGcJob),
        Box::new(DocumentCollabReaperJob),
        Box::new(DocumentCollabUpdateSweepJob),
        Box::new(DocumentMediaBackfillJob),
        Box::new(DocumentNodeIdsBackfillJob),
        Box::new(DocumentPresenceSweepJob),
        Box::new(DocumentUploadExpiryJob),
    ];
    let mut names = Vec::new();
    for job in jobs {
        assert_eq!(job.scope(), JobScope::Cluster, "{}", job.name());
        assert_eq!(job.schedule().split(' ').count(), 6, "{}", job.name());
        assert!(!job.description().is_empty());
        let result = job.execute(&ctx).await.expect("disabled run");
        assert!(result.success);
        assert_eq!(
            result.message.as_deref(),
            Some("document store disabled"),
            "{}",
            job.name()
        );
        names.push(job.name());
    }
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 8);
}

#[tokio::test]
async fn the_presence_sweep_removes_expired_presence_and_keeps_live_presence() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let (document, _) = memo(&h).await;
    exec(
        "INSERT INTO document_presence (document_id, user_id, source, state, expires_at) VALUES \
         ($1::uuid, 'gone', 'web', 'viewing', now() - interval '1 minute'), \
         ($1::uuid, 'here', 'web', 'editing', now() + interval '1 hour')",
        document.as_str(),
    )
    .await;

    let result = DocumentPresenceSweepJob
        .execute(&context(test_db_pool().await, true, None))
        .await
        .expect("sweep");
    assert!(result.success);
    assert!(result.items_processed.expect("stats") >= 1);
    assert_eq!(result.items_failed, Some(0));

    let remaining: Vec<String> = {
        let pool = test_db_pool().await.pool();
        sqlx::query_scalar("SELECT user_id FROM document_presence WHERE document_id = $1::uuid")
            .bind(document.as_str())
            .fetch_all(&*pool)
            .await
            .expect("remaining")
    };
    assert_eq!(remaining, vec!["here".to_owned()]);
}

#[tokio::test]
async fn agent_read_retention_purges_rows_past_the_window_only() {
    let document = Uuid::new_v4().to_string();
    exec(
        "INSERT INTO document_agent_reads (document_id, user_id, agent_name, attended, action, \
         effective_cap, cap_source, created_at, read_day) VALUES \
         ($1::uuid, 'u', 'old-agent', true, 'read', 'read', 'test', now() - interval '400 days', \
          (now() - interval '400 days')::date), \
         ($1::uuid, 'u', 'new-agent', true, 'read', 'read', 'test', now(), now()::date)",
        &document,
    )
    .await;

    let result = DocumentAgentReadRetentionJob
        .execute(&context(test_db_pool().await, true, None))
        .await
        .expect("retention");
    assert!(result.success);
    assert!(result.items_processed.expect("stats") >= 1);

    let agents: Vec<String> = {
        let pool = test_db_pool().await.pool();
        sqlx::query_scalar(
            "SELECT agent_name FROM document_agent_reads WHERE document_id = $1::uuid",
        )
        .bind(&document)
        .fetch_all(&*pool)
        .await
        .expect("agents")
    };
    assert_eq!(agents, vec!["new-agent".to_owned()]);
}

#[tokio::test]
async fn the_collab_reaper_stands_down_without_live_co_editing() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let result = DocumentCollabReaperJob
        .execute(&context(test_db_pool().await, true, Some(&h.service)))
        .await
        .expect("reaper");
    assert!(result.success);
    assert_eq!(
        result.message.as_deref(),
        Some("live co-editing not attached")
    );
}

#[tokio::test]
async fn the_collab_update_sweep_reports_its_stats() {
    let result = DocumentCollabUpdateSweepJob
        .execute(&context(test_db_pool().await, true, None))
        .await
        .expect("sweep");
    assert!(result.success);
    assert!(result.items_processed.is_some());
    assert_eq!(result.items_failed, Some(0));
}

#[tokio::test]
async fn the_backfill_job_mints_missing_ids_once_per_release() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let (_, revision) = memo(&h).await;
    let ids = "SELECT COUNT(*) FROM document_node_ids WHERE revision_id = $1::uuid";
    assert!(count(ids, revision.as_str()).await > 0);
    exec(
        "DELETE FROM document_node_ids WHERE revision_id = $1::uuid",
        revision.as_str(),
    )
    .await;
    exec(
        "DELETE FROM release_stamps WHERE name = $1",
        "document_node_ids_backfill",
    )
    .await;

    let ctx = context(test_db_pool().await, true, Some(&h.service));
    let first = DocumentNodeIdsBackfillJob
        .execute(&ctx)
        .await
        .expect("first run");
    assert!(first.success);
    assert!(first.items_processed.expect("ran") >= 1);
    assert!(count(ids, revision.as_str()).await > 0);

    let second = DocumentNodeIdsBackfillJob
        .execute(&ctx)
        .await
        .expect("second run");
    assert_eq!(
        second.message.as_deref(),
        Some("node ids already backfilled for this release")
    );
}
