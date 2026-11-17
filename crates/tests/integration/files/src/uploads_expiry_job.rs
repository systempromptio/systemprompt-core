use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionContext;
use systemprompt_files::uploads::{
    DocumentUploadExpiryJob, DocumentUploadRepository, UploadStatus,
};
use systemprompt_files::{DocumentsConfig, FilesConfig, FilesConfigYaml, UploadLimits};
use systemprompt_identifiers::{DocumentUploadId, UserId};
use systemprompt_provider_contracts::{Job, JobContext, JobScope};
use systemprompt_test_fixtures::{DisposableDb, fixture_actor, test_app_context};

use crate::uploads_rig::{request, rig_on, trace};

fn enabled() -> Arc<FilesConfig> {
    let yaml = FilesConfigYaml {
        documents: DocumentsConfig {
            enabled: true,
            ..DocumentsConfig::default()
        },
        ..FilesConfigYaml::default()
    };
    Arc::new(FilesConfig::new(PathBuf::from("/tmp"), yaml))
}

async fn status_of(db: &DbPool, id: &DocumentUploadId) -> UploadStatus {
    DocumentUploadRepository::new(db)
        .find(id)
        .await
        .expect("find")
        .expect("row")
        .status
}

#[test]
fn the_expiry_job_runs_every_quarter_hour_on_one_replica() {
    assert_eq!(DocumentUploadExpiryJob.name(), "document_upload_expiry");
    assert_eq!(DocumentUploadExpiryJob.schedule(), "0 */15 * * * *");
    assert_eq!(DocumentUploadExpiryJob.scope(), JobScope::Cluster);
    assert!(DocumentUploadExpiryJob.description().contains("uploads"));
}

#[tokio::test]
async fn the_enabled_expiry_job_closes_an_upload_past_its_lifetime() {
    let disposable = DisposableDb::with_schema("files_upload_expiry_job").await;
    let db = disposable.test_pool().await;
    let rig = rig_on(&db, UploadLimits::default());
    let uploader = UserId::new("wp10-expiry-job-uploader");
    let stale = rig
        .service
        .create(
            &uploader,
            request("stale.txt", "text/plain", b"abcd"),
            &trace(),
        )
        .await
        .expect("create stale");
    let live = rig
        .service
        .create(
            &uploader,
            request("live.txt", "text/plain", b"efgh"),
            &trace(),
        )
        .await
        .expect("create live");
    sqlx::query(
        "UPDATE document_uploads SET expires_at = now() - interval '1 hour' WHERE id = $1::uuid",
    )
    .bind(stale.id.as_str())
    .execute(&*db.pool())
    .await
    .expect("backdate");
    let app = test_app_context(disposable.url()).await;
    let extensions: Arc<dyn ExtensionContext> = app.clone();
    let ctx = JobContext::new(
        fixture_actor(),
        app.kernel()
            .state()
            .clone()
            .with(Arc::clone(&db))
            .with(enabled())
            .with(extensions),
    );

    let result = DocumentUploadExpiryJob
        .execute(&ctx)
        .await
        .expect("job runs");

    assert!(result.success, "{result:?}");
    assert!(result.items_processed.is_some_and(|n| n >= 1), "{result:?}");
    let message = result.message.expect("a summary");
    assert!(message.starts_with("Expired: "), "{message}");
    assert!(message.contains("orphans deleted: "), "{message}");
    assert_eq!(status_of(&db, &stale.id).await, UploadStatus::Expired);
    assert_eq!(status_of(&db, &live.id).await, UploadStatus::Open);
    disposable.drop_now().await;
}
