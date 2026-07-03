//! DB-backed tests for [`McpArtifactRepository`].

use systemprompt_identifiers::{ArtifactId, ContextId, McpExecutionId, UserId};
use systemprompt_mcp::repository::{CreateMcpArtifact, McpArtifactRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn full_artifact(id: &ArtifactId, server: &str) -> CreateMcpArtifact {
    CreateMcpArtifact {
        artifact_id: id.clone(),
        mcp_execution_id: McpExecutionId::new(unique("exec")),
        context_id: Some(ContextId::new("00000000-0000-4000-8000-000000000abc")),
        user_id: Some(UserId::new("11111111-1111-4111-8111-111111111abc")),
        server_name: server.to_owned(),
        artifact_type: "document".to_owned(),
        title: Some("Report".to_owned()),
        data: serde_json::json!({"body": "hello"}),
        metadata: Some(serde_json::json!({"k": "v"})),
        expires_at: None,
    }
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

#[tokio::test]
async fn save_then_find_round_trips_all_fields() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let server = unique("srv");
    let create = full_artifact(&id, &server);
    repo.save(&create).await.unwrap();

    let found = repo
        .find_by_id(&id)
        .await
        .unwrap()
        .expect("saved artifact is found");
    assert_eq!(found.artifact_id, id);
    assert_eq!(found.server_name, server);
    assert_eq!(found.artifact_type, "document");
    assert_eq!(found.title.as_deref(), Some("Report"));
    assert_eq!(found.data, serde_json::json!({"body": "hello"}));
    assert_eq!(found.metadata, Some(serde_json::json!({"k": "v"})));
    assert_eq!(
        found.context_id,
        Some(ContextId::new("00000000-0000-4000-8000-000000000abc"))
    );
    assert!(found.expires_at.is_none());
}

#[tokio::test]
async fn save_on_conflict_updates_mutable_fields() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let server = unique("srv");
    repo.save(&full_artifact(&id, &server)).await.unwrap();

    let mut updated = full_artifact(&id, &server);
    updated.title = Some("Revised".to_owned());
    updated.data = serde_json::json!({"body": "world"});
    updated.metadata = Some(serde_json::json!({"k": "v2"}));
    repo.save(&updated).await.unwrap();

    let found = repo.find_by_id(&id).await.unwrap().expect("still present");
    assert_eq!(found.title.as_deref(), Some("Revised"));
    assert_eq!(found.data, serde_json::json!({"body": "world"}));
    assert_eq!(found.metadata, Some(serde_json::json!({"k": "v2"})));
}

#[tokio::test]
async fn list_by_server_returns_saved_rows() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let server = unique("srv");
    let id_a = ArtifactId::new(unique("art"));
    let id_b = ArtifactId::new(unique("art"));
    repo.save(&full_artifact(&id_a, &server)).await.unwrap();
    repo.save(&full_artifact(&id_b, &server)).await.unwrap();

    let rows = repo.list_by_server(&server, 10).await.unwrap();
    assert_eq!(
        rows.len(),
        2,
        "both artifacts for this unique server listed"
    );
    let ids: Vec<&ArtifactId> = rows.iter().map(|r| &r.artifact_id).collect();
    assert!(ids.contains(&&id_a));
    assert!(ids.contains(&&id_b));
}

#[tokio::test]
async fn delete_returns_true_for_existing_artifact() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    repo.save(&full_artifact(&id, &unique("srv")))
        .await
        .unwrap();

    assert!(repo.delete(&id).await.unwrap(), "existing artifact deleted");
    assert!(
        repo.find_by_id(&id).await.unwrap().is_none(),
        "artifact gone after delete"
    );
}

#[tokio::test]
async fn find_by_id_hides_expired_artifact() {
    use chrono::{Duration, Utc};

    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let mut create = full_artifact(&id, &unique("srv"));
    create.expires_at = Some(Utc::now() - Duration::hours(1));
    repo.save(&create).await.unwrap();

    assert!(
        repo.find_by_id(&id).await.unwrap().is_none(),
        "a past-due artifact is filtered out of find_by_id"
    );
}
