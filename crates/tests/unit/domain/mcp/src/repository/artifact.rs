//! DB-backed tests for [`McpArtifactRepository`].

use systemprompt_identifiers::ArtifactId;
use systemprompt_mcp::repository::McpArtifactRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = db().await else { return };
    drop(McpArtifactRepository::new(&db).expect("ctor"));
}

#[tokio::test]
async fn find_by_id_random_returns_none() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(format!("art-{}", uuid::Uuid::new_v4().simple()));
    let r = repo.find_by_id(&id).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn list_by_server_returns_vec() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let r = repo
        .list_by_server(&format!("none-{}", uuid::Uuid::new_v4().simple()), 10)
        .await
        .unwrap();
    assert!(r.is_empty());
}

#[tokio::test]
async fn delete_random_returns_false() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(format!("art-{}", uuid::Uuid::new_v4().simple()));
    let ok = repo.delete(&id).await.unwrap();
    assert!(!ok);
}

#[tokio::test]
async fn cleanup_expired_reaps_a_past_due_artifact() {
    use chrono::{Duration, Utc};
    use systemprompt_identifiers::McpExecutionId;
    use systemprompt_mcp::repository::CreateMcpArtifact;

    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(format!("art-{}", uuid::Uuid::new_v4().simple()));
    repo.save(&CreateMcpArtifact {
        artifact_id: id.clone(),
        mcp_execution_id: McpExecutionId::new(format!("exec-{}", uuid::Uuid::new_v4().simple())),
        context_id: None,
        user_id: None,
        server_name: "art-cleanup".to_owned(),
        artifact_type: "text".to_owned(),
        title: None,
        data: serde_json::json!({"k": "v"}),
        metadata: None,
        expires_at: Some(Utc::now() - Duration::hours(1)),
    })
    .await
    .unwrap();

    let reaped = repo.cleanup_expired().await.unwrap();
    assert!(
        reaped >= 1,
        "cleanup_expired deletes at least the seeded past-due artifact"
    );
    assert!(
        repo.find_by_id(&id).await.unwrap().is_none(),
        "the seeded artifact no longer exists after cleanup_expired"
    );
}
