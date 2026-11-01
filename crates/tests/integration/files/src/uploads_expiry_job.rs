use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_extension::ExtensionContext;
use systemprompt_files::uploads::{
    DocumentUploadExpiryJob, DocumentUploadRepository, UploadStatus,
};
use systemprompt_files::{DocumentsConfig, FilesConfig, FilesConfigYaml};
use systemprompt_identifiers::{DocumentUploadId, UserId};
use systemprompt_provider_contracts::{Dependencies, Job, JobContext, JobScope};
use systemprompt_test_fixtures::{
    fixture_actor, test_app_context, test_database_url, test_db_pool,
};

use crate::uploads_rig::{request, rig, trace};

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

async fn status_of(id: &DocumentUploadId) -> UploadStatus {
    DocumentUploadRepository::new(&test_db_pool().await)
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
    let rig = rig().await;
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
    let db = test_db_pool().await;
    sqlx::query(
        "UPDATE document_uploads SET expires_at = now() - interval '1 hour' WHERE id = $1::uuid",
    )
    .bind(stale.id.as_str())
    .execute(&*db.pool())
    .await
    .expect("backdate");
    let extensions: Arc<dyn ExtensionContext> = test_app_context(&db, &test_database_url());
    let ctx = JobContext::new(
        fixture_actor(),
        Dependencies::new()
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
    assert_eq!(status_of(&stale.id).await, UploadStatus::Expired);
    assert_eq!(status_of(&live.id).await, UploadStatus::Open);
}
