//! Issue reports against a migrated database: folding by fingerprint, the
//! hourly rate limit, visibility, triage transitions, attachments, the blob
//! collector's hold on attached blobs, retention and the `issue.reported`
//! outbox fact.

use systemprompt_database::DbPool;
use systemprompt_files::IssuesConfig;
use systemprompt_files::documents::repository::DocumentBlobRepository;
use systemprompt_files::issues::{
    IssueFiling, IssueReportRepository, IssueRetentionJob, IssueService, IssuesError,
};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, IssueReportId, UserId};
use systemprompt_models::issues::{IssueContext, IssueKind, IssueSeverity, IssueStatus};
use systemprompt_models::notifications::{NOTIFICATIONS_CONSUMER, NotificationKind};
use systemprompt_test_fixtures::{fixture_notification_sink, test_db_pool};

fn enabled() -> IssuesConfig {
    IssuesConfig {
        enabled: true,
        ..IssuesConfig::default()
    }
}

fn service(db: &DbPool, config: IssuesConfig) -> IssueService {
    IssueService::new(
        IssueReportRepository::new(db),
        fixture_notification_sink(db),
        config,
    )
}

fn reporter(label: &str) -> UserId {
    UserId::new(format!("issues-{label}-{}", uuid::Uuid::new_v4()))
}

fn filing(reporter: &UserId, title: &str) -> IssueFiling {
    IssueFiling {
        reporter: reporter.clone(),
        via_agent: None,
        kind: IssueKind::Bug,
        severity: IssueSeverity::Medium,
        title: title.to_owned(),
        body: "steps to reproduce".to_owned(),
        fingerprint_key: None,
        context: IssueContext::default(),
        client: serde_json::Value::Null,
    }
}

async fn rows_for(db: &DbPool, reporter: &UserId) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM issue_reports WHERE reporter_user_id = $1")
        .bind(reporter.as_str())
        .fetch_one(db.write_pool().as_ref())
        .await
        .expect("count reports")
}

async fn reported_facts(db: &DbPool, id: &IssueReportId) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_outbox \
         WHERE consumer = $1 AND channel = 'fact' \
         AND fact->'data'->>'kind' = $2 AND fact::text LIKE '%' || $3 || '%'",
    )
    .bind(NOTIFICATIONS_CONSUMER)
    .bind(NotificationKind::ISSUE_REPORTED)
    .bind(id.as_str())
    .fetch_one(db.write_pool().as_ref())
    .await
    .expect("count facts")
}

fn random_hash() -> ContentHash {
    let hex = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    ContentHash::try_new(hex).expect("hash")
}

async fn upload(db: &DbPool, owner: &UserId, status: &str) -> (DocumentUploadId, ContentHash) {
    let hash = random_hash();
    DocumentBlobRepository::new(db)
        .upsert(&hash, 128, &format!("issues-test/{}", hash.as_str()))
        .await
        .expect("blob");
    let id: String = sqlx::query_scalar(
        "INSERT INTO document_uploads \
             (owner_user_id, quota_user_id, file_name, declared_mime, declared_size, declared_sha256, \
              part_size, status, blob_sha256, detected_mime, trace_id, expires_at, finished_at) \
         VALUES ($1, $1, 'screen.png', 'image/png', 128, $2, 128, $3, \
                 CASE WHEN $3 = 'completed' THEN $2 END, 'image/png', 'issues-test', \
                 now() + interval '1 hour', now()) \
         RETURNING id::text",
    )
    .bind(owner.as_str())
    .bind(hash.as_str())
    .bind(status)
    .fetch_one(db.write_pool().as_ref())
    .await
    .expect("upload");
    (DocumentUploadId::try_new(id).expect("upload id"), hash)
}

#[tokio::test]
async fn a_repeat_of_an_open_report_folds_into_it() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("fold");

    let (first, filed) = issues
        .file(filing(&who, "Export hangs"))
        .await
        .expect("file");
    assert!(filed.inserted);
    assert_eq!(filed.occurrences, 1);

    let (second, folded) = issues
        .file(filing(&who, "  export   HANGS "))
        .await
        .expect("repeat");
    assert_eq!(second, first);
    assert!(!folded.inserted);
    assert_eq!(folded.occurrences, 2);
    assert_eq!(rows_for(&db, &who).await, 1);
}

#[tokio::test]
async fn a_resolved_report_no_longer_folds() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("resolved");
    let triager = reporter("resolved-triager");

    let (first, _) = issues.file(filing(&who, "Login loop")).await.expect("file");
    issues
        .set_status(&first, IssueStatus::Resolved, None, &triager)
        .await
        .expect("resolve");

    let (second, filed) = issues
        .file(filing(&who, "Login loop"))
        .await
        .expect("repeat");
    assert_ne!(second, first);
    assert!(filed.inserted);
    assert_eq!(filed.occurrences, 1);
    assert_eq!(rows_for(&db, &who).await, 2);
}

#[tokio::test]
async fn the_report_past_the_hourly_limit_is_refused_and_leaves_no_row() {
    let db = test_db_pool().await;
    let issues = service(
        &db,
        IssuesConfig {
            max_reports_per_hour: 2,
            ..enabled()
        },
    );
    let who = reporter("rate");

    issues.file(filing(&who, "first")).await.expect("first");
    issues.file(filing(&who, "second")).await.expect("second");
    let (_, folded) = issues.file(filing(&who, "first")).await.expect("fold");
    assert!(!folded.inserted, "a fold is not counted against the limit");

    let refused = issues.file(filing(&who, "third")).await;
    assert!(
        matches!(refused, Err(IssuesError::RateLimited { limit: 2 })),
        "{refused:?}"
    );
    assert_eq!(rows_for(&db, &who).await, 2);
}

#[tokio::test]
async fn a_report_is_visible_to_its_reporter_and_triagers_only() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("visible");
    let stranger = reporter("visible-stranger");
    let (id, _) = issues
        .file(filing(&who, "Broken chart"))
        .await
        .expect("file");

    let seen = issues
        .find_visible(&id, &who, &[])
        .await
        .expect("reporter sees it");
    assert_eq!(seen.title, "Broken chart");

    let refused = issues
        .find_visible(&id, &stranger, &["user".to_owned()])
        .await;
    assert!(
        matches!(refused, Err(IssuesError::NotFound(_))),
        "{refused:?}"
    );

    let triaged = issues
        .find_visible(&id, &stranger, &["admin".to_owned()])
        .await
        .expect("admin sees it");
    assert_eq!(triaged.id, id);
}

#[tokio::test]
async fn a_status_change_reports_its_prior_status_and_tracks_closed_at() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("status");
    let triager = reporter("status-triager");
    let (id, _) = issues.file(filing(&who, "Typo")).await.expect("file");

    let from = issues
        .set_status(&id, IssueStatus::Triaged, Some("seen".to_owned()), &triager)
        .await
        .expect("triage");
    assert_eq!(from, IssueStatus::Open);

    let from = issues
        .set_status(&id, IssueStatus::Resolved, None, &triager)
        .await
        .expect("resolve");
    assert_eq!(from, IssueStatus::Triaged);
    let resolved = issues.find(&id).await.expect("find").expect("report");
    assert_eq!(resolved.status, IssueStatus::Resolved);
    assert!(resolved.closed_at.is_some());
    assert_eq!(resolved.triage_note.as_deref(), Some("seen"));
    assert_eq!(resolved.triaged_by.as_ref(), Some(&triager));

    let from = issues
        .set_status(&id, IssueStatus::Open, None, &triager)
        .await
        .expect("reopen");
    assert_eq!(from, IssueStatus::Resolved);
    let reopened = issues.find(&id).await.expect("find").expect("report");
    assert_eq!(reopened.status, IssueStatus::Open);
    assert!(reopened.closed_at.is_none());

    let missing = issues
        .set_status(
            &IssueReportId::generate(),
            IssueStatus::Closed,
            None,
            &triager,
        )
        .await;
    assert!(
        matches!(missing, Err(IssuesError::NotFound(_))),
        "{missing:?}"
    );
}

#[tokio::test]
async fn only_the_reporters_completed_uploads_attach() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("attach");
    let stranger = reporter("attach-stranger");
    let (id, _) = issues
        .file(filing(&who, "Crash on save"))
        .await
        .expect("file");

    let (own, own_hash) = upload(&db, &who, "completed").await;
    issues.attach(&id, &own, &who).await.expect("attach own");
    issues
        .attach(&id, &own, &who)
        .await
        .expect("re-attaching the same upload is a no-op");

    let (theirs, _) = upload(&db, &stranger, "completed").await;
    let refused = issues.attach(&id, &theirs, &who).await;
    assert!(
        matches!(refused, Err(IssuesError::UploadNotReady(ref u)) if *u == theirs),
        "{refused:?}"
    );

    let (unfinished, _) = upload(&db, &who, "open").await;
    let refused = issues.attach(&id, &unfinished, &who).await;
    assert!(
        matches!(refused, Err(IssuesError::UploadNotReady(_))),
        "{refused:?}"
    );

    let report = issues.find(&id).await.expect("find").expect("report");
    assert_eq!(report.attachments.len(), 1);
    assert_eq!(report.attachments[0].upload_id, own);
    assert_eq!(report.attachments[0].content_sha256, own_hash);
}

#[tokio::test]
async fn a_report_holds_at_most_its_attachment_cap() {
    let db = test_db_pool().await;
    let issues = service(
        &db,
        IssuesConfig {
            max_attachments: 2,
            ..enabled()
        },
    );
    let who = reporter("cap");
    let (id, _) = issues
        .file(filing(&who, "Many screenshots"))
        .await
        .expect("file");

    for _ in 0..2 {
        let (next, _) = upload(&db, &who, "completed").await;
        issues.attach(&id, &next, &who).await.expect("attach");
    }
    let (extra, _) = upload(&db, &who, "completed").await;
    let refused = issues.attach(&id, &extra, &who).await;
    assert!(
        matches!(refused, Err(IssuesError::AttachmentsFull { limit: 2, .. })),
        "{refused:?}"
    );
    let report = issues.find(&id).await.expect("find").expect("report");
    assert_eq!(report.attachments.len(), 2);
}

#[tokio::test]
async fn a_resolved_report_takes_no_attachments() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("inactive");
    let (id, _) = issues
        .file(filing(&who, "Done already"))
        .await
        .expect("file");
    issues
        .set_status(&id, IssueStatus::Closed, None, &who)
        .await
        .expect("close");
    let (own, _) = upload(&db, &who, "completed").await;
    let refused = issues.attach(&id, &own, &who).await;
    assert!(
        matches!(refused, Err(IssuesError::NotActive(_))),
        "{refused:?}"
    );
}

#[tokio::test]
async fn the_blob_collector_keeps_an_attached_blob_until_its_report_is_purged() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let blobs = DocumentBlobRepository::new(&db);
    let who = reporter("gc");
    let (id, _) = issues
        .file(filing(&who, "Corrupt export"))
        .await
        .expect("file");
    let (own, hash) = upload(&db, &who, "completed").await;
    issues.attach(&id, &own, &who).await.expect("attach");
    sqlx::query("UPDATE document_blobs SET created_at = now() - interval '30 days' WHERE content_sha256 = $1")
        .bind(hash.as_str())
        .execute(db.write_pool().as_ref())
        .await
        .expect("age blob");

    let candidates = blobs
        .unreferenced_older_than(24, 100_000)
        .await
        .expect("candidates");
    assert!(
        !candidates.iter().any(|blob| blob.content_sha256 == hash),
        "an attached blob is not a collection candidate"
    );
    assert!(
        !blobs.delete(&hash).await.expect("delete"),
        "attached blob is kept"
    );

    issues
        .set_status(&id, IssueStatus::Resolved, None, &who)
        .await
        .expect("resolve");
    sqlx::query(
        "UPDATE issue_reports SET closed_at = now() - interval '91 days' WHERE id = $1::text::uuid",
    )
    .bind(id.as_str())
    .execute(db.write_pool().as_ref())
    .await
    .expect("age report");
    let purged = IssueRetentionJob::purge(&db, 90).await.expect("purge");
    assert!(purged >= 1);
    assert!(issues.find(&id).await.expect("find").is_none());

    assert!(
        blobs.delete(&hash).await.expect("delete"),
        "released blob is collected"
    );
    assert!(blobs.find_on_primary(&hash).await.expect("find").is_none());
}

#[tokio::test]
async fn retention_keeps_reports_closed_inside_the_window() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("retain");
    let (id, _) = issues.file(filing(&who, "Recent fix")).await.expect("file");
    issues
        .set_status(&id, IssueStatus::Closed, None, &who)
        .await
        .expect("close");
    IssueRetentionJob::purge(&db, 90).await.expect("purge");
    assert!(issues.find(&id).await.expect("find").is_some());
}

#[tokio::test]
async fn a_disabled_store_refuses_every_call() {
    let db = test_db_pool().await;
    let issues = service(&db, IssuesConfig::default());
    let who = reporter("disabled");

    assert!(matches!(
        issues.file(filing(&who, "nope")).await,
        Err(IssuesError::Disabled)
    ));
    assert!(matches!(
        issues
            .find_visible(&IssueReportId::generate(), &who, &[])
            .await,
        Err(IssuesError::Disabled)
    ));
    assert!(matches!(
        issues
            .set_status(&IssueReportId::generate(), IssueStatus::Closed, None, &who)
            .await,
        Err(IssuesError::Disabled)
    ));
    assert_eq!(rows_for(&db, &who).await, 0);
}

#[tokio::test]
async fn a_new_report_emits_one_fact_and_a_fold_emits_none() {
    let db = test_db_pool().await;
    let issues = service(&db, enabled());
    let who = reporter("facts");

    let (id, _) = issues.file(filing(&who, "Fact check")).await.expect("file");
    assert_eq!(reported_facts(&db, &id).await, 1);

    let (again, folded) = issues.file(filing(&who, "Fact check")).await.expect("fold");
    assert_eq!(again, id);
    assert!(!folded.inserted);
    assert_eq!(reported_facts(&db, &id).await, 1);
}
