//! Reads a user route makes by id answer only through the caller: an upload
//! session and an issue report owned by someone else answer exactly as an
//! id that was never minted.

use systemprompt_database::DbPool;
use systemprompt_files::IssuesConfig;
use systemprompt_files::issues::{IssueFiling, IssueReportRepository, IssueService, IssuesError};
use systemprompt_files::uploads::DocumentUploadRepository;
use systemprompt_identifiers::{DocumentUploadId, IssueReportId, UserId};
use systemprompt_models::issues::{IssueContext, IssueKind, IssueSeverity};
use systemprompt_test_fixtures::{fixture_notification_sink, test_db_pool};

fn user(label: &str) -> UserId {
    UserId::new(format!(
        "owner-scoped-{label}-{}",
        uuid::Uuid::new_v4().simple()
    ))
}

async fn open_upload(db: &DbPool, owner: &UserId) -> DocumentUploadId {
    let id: String = sqlx::query_scalar(
        "INSERT INTO document_uploads \
             (owner_user_id, quota_user_id, file_name, declared_mime, declared_size, \
              declared_sha256, part_size, status, trace_id, expires_at) \
         VALUES ($1, $1, 'notes.txt', 'text/plain', 4, \
                 repeat('a', 64), 4, 'open', 'owner-scoped-test', now() + interval '1 hour') \
         RETURNING id::text",
    )
    .bind(owner.as_str())
    .fetch_one(db.write_pool().as_ref())
    .await
    .expect("upload");
    DocumentUploadId::try_new(id).expect("upload id")
}

fn issues(db: &DbPool) -> IssueService {
    IssueService::new(
        IssueReportRepository::new(db),
        fixture_notification_sink(db),
        IssuesConfig {
            enabled: true,
            ..IssuesConfig::default()
        },
    )
}

fn filing(reporter: &UserId) -> IssueFiling {
    IssueFiling {
        reporter: reporter.clone(),
        via_agent: None,
        kind: IssueKind::Bug,
        severity: IssueSeverity::Medium,
        title: format!("Owner scoped {}", uuid::Uuid::new_v4().simple()),
        body: "steps to reproduce".to_owned(),
        fingerprint_key: None,
        context: IssueContext::default(),
        client: serde_json::Value::Null,
    }
}

#[tokio::test]
async fn a_foreign_upload_session_answers_as_an_unknown_one() {
    let db = test_db_pool().await;
    let uploads = DocumentUploadRepository::new(&db);
    let owner = user("uploader");
    let stranger = user("stranger");
    let id = open_upload(&db, &owner).await;

    let own = uploads.find_owned(&id, &owner).await.expect("own read");
    assert_eq!(own.expect("own session").owner_user_id, owner);

    let foreign = uploads
        .find_owned(&id, &stranger)
        .await
        .expect("foreign read");
    let unknown = uploads
        .find_owned(&DocumentUploadId::generate(), &stranger)
        .await
        .expect("unknown read");
    assert!(foreign.is_none());
    assert_eq!(foreign.is_none(), unknown.is_none());
    assert!(uploads.find(&id).await.expect("job read").is_some());
}

#[tokio::test]
async fn a_foreign_issue_report_answers_as_an_unknown_one() {
    let db = test_db_pool().await;
    let repo = IssueReportRepository::new(&db);
    let service = issues(&db);
    let reporter = user("reporter");
    let stranger = user("stranger");
    let (id, _) = service.file(filing(&reporter)).await.expect("file");

    let own = repo
        .find_reported_by(&id, Some(&reporter))
        .await
        .expect("own read");
    assert_eq!(own.expect("own report").reporter_user_id, reporter);
    assert!(
        repo.find_reported_by(&id, None)
            .await
            .expect("triage read")
            .is_some()
    );

    let foreign = repo
        .find_reported_by(&id, Some(&stranger))
        .await
        .expect("foreign read");
    let unknown = repo
        .find_reported_by(&IssueReportId::generate(), Some(&stranger))
        .await
        .expect("unknown read");
    assert!(foreign.is_none());
    assert_eq!(foreign.is_none(), unknown.is_none());

    let foreign = service.find_visible(&id, &stranger, &[]).await;
    let unknown = service
        .find_visible(&IssueReportId::generate(), &stranger, &[])
        .await;
    assert!(
        matches!(foreign, Err(IssuesError::NotFound(_))),
        "{foreign:?}"
    );
    assert!(
        matches!(unknown, Err(IssuesError::NotFound(_))),
        "{unknown:?}"
    );
}
