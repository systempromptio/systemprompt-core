//! DB-backed tests for the payload sweep an artifact delete carries: a body
//! nothing references any more goes with the artifact, a body another
//! artifact still points at stays, and a body seen within the grace window
//! is left for an in-flight ingest to link.

use systemprompt_identifiers::ArtifactId;
use systemprompt_mcp::repository::{
    ArtifactPayloadRepository, CreateMcpArtifact, McpArtifactRepository,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

use super::artifact::seed_execution;

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn digest() -> String {
    format!("{:0>64}", uuid::Uuid::new_v4().simple())
}

async fn linked_artifact(
    db: &systemprompt_database::DbPool,
    payloads: &ArtifactPayloadRepository,
    artifacts: &McpArtifactRepository,
    sha256: &str,
) -> ArtifactId {
    payloads
        .upsert_payload(sha256, 2, &serde_json::json!({}))
        .await
        .expect("upsert payload");
    let id = ArtifactId::new(unique("art"));
    let exec = seed_execution(db, "sweep-tests").await;
    let mut create = CreateMcpArtifact::new(
        id.clone(),
        exec,
        "sweep-tests",
        "tool_result",
        serde_json::json!({}),
    );
    create.payload_sha256 = Some(sha256.to_owned());
    artifacts.save(&create).await.expect("save artifact");
    id
}

async fn age_payload(db: &systemprompt_database::DbPool, sha256: &str) {
    let raw = db.pool_arc().expect("raw pool");
    sqlx::query(
        "UPDATE artifact_payloads SET last_seen_at = NOW() - interval '2 hours' WHERE sha256 = $1",
    )
    .bind(sha256)
    .execute(raw.as_ref())
    .await
    .expect("age payload");
}

#[tokio::test]
async fn deleting_the_last_artifact_removes_its_body_and_spares_a_shared_one() {
    let Some(db) = db_or_skip().await else { return };
    let artifacts = McpArtifactRepository::new(&db).unwrap();
    let payloads = ArtifactPayloadRepository::new(&db).unwrap();
    let sole = digest();
    let shared = digest();
    let sole_artifact = linked_artifact(&db, &payloads, &artifacts, &sole).await;
    let shared_a = linked_artifact(&db, &payloads, &artifacts, &shared).await;
    linked_artifact(&db, &payloads, &artifacts, &shared).await;
    age_payload(&db, &sole).await;
    age_payload(&db, &shared).await;

    assert!(artifacts.delete(&sole_artifact).await.unwrap());
    assert!(
        !payloads.payload_exists(&sole).await.unwrap(),
        "the only reference leaving takes the body with it"
    );

    assert!(artifacts.delete(&shared_a).await.unwrap());
    assert!(
        payloads.payload_exists(&shared).await.unwrap(),
        "a body another artifact still references stays"
    );
}

#[tokio::test]
async fn a_body_seen_within_the_grace_window_is_not_swept() {
    let Some(db) = db_or_skip().await else { return };
    let artifacts = McpArtifactRepository::new(&db).unwrap();
    let payloads = ArtifactPayloadRepository::new(&db).unwrap();
    let in_flight = digest();
    payloads
        .upsert_payload(&in_flight, 2, &serde_json::json!({}))
        .await
        .expect("upsert payload");
    let victim = linked_artifact(&db, &payloads, &artifacts, &digest()).await;

    assert!(artifacts.delete(&victim).await.unwrap());
    assert!(
        payloads.payload_exists(&in_flight).await.unwrap(),
        "a body an ingest just wrote is left for it to link"
    );
}
