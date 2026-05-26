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
    let _ = McpArtifactRepository::new(&db).expect("ctor");
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
async fn cleanup_expired_executes() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let _ = repo.cleanup_expired().await.unwrap();
}
