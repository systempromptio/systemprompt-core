use rmcp::model::ReadResourceRequestParams;
use systemprompt_identifiers::{ArtifactId, ContextId};
use systemprompt_mcp::read_artifact_resource;
use systemprompt_mcp::repository::{CreateMcpArtifact, McpArtifactRepository};
use systemprompt_test_fixtures::test_db_pool;

async fn repo() -> (systemprompt_database::DbPool, McpArtifactRepository) {
    let db = test_db_pool().await;
    let repo = McpArtifactRepository::new(&db).expect("artifact repository");
    (db, repo)
}

fn fresh_id() -> ArtifactId {
    ArtifactId::new(format!("art-{}", uuid::Uuid::new_v4().simple()))
}

async fn stored(
    db: &systemprompt_database::DbPool,
    id: &ArtifactId,
    data: serde_json::Value,
    context_id: Option<ContextId>,
) -> CreateMcpArtifact {
    let exec = crate::repository::artifact::seed_execution(db, "res-tests").await;
    let mut create = CreateMcpArtifact::new(id.clone(), exec, "res-tests", "message", data);
    create.context_id = context_id;
    create.title = Some("Stored Message".to_owned());
    create
}

#[tokio::test]
async fn read_artifact_rejects_non_artifact_uri() {
    let (_db, repo) = repo().await;
    let request = ReadResourceRequestParams::new("ui://srv/artifact-viewer");
    let err = read_artifact_resource(&request, "srv", &repo)
        .await
        .unwrap_err();
    assert!(err.message.contains("Not an artifact resource URI"));
}

#[tokio::test]
async fn read_artifact_rejects_server_mismatch() {
    let (_db, repo) = repo().await;
    let request = ReadResourceRequestParams::new("ui://other/artifact/abc");
    let err = read_artifact_resource(&request, "srv", &repo)
        .await
        .unwrap_err();
    assert!(err.message.contains("names server 'other'"));
    assert!(err.message.contains("'srv'"));
}

#[tokio::test]
async fn read_artifact_unknown_id_is_invalid_params() {
    let (_db, repo) = repo().await;
    let id = fresh_id();
    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let err = read_artifact_resource(&request, "srv", &repo)
        .await
        .unwrap_err();
    assert!(err.message.contains("Unknown artifact"));
}

#[tokio::test]
async fn read_artifact_without_payload_key_is_internal_error() {
    let (db, repo) = repo().await;
    let id = fresh_id();
    repo.save(&stored(&db, &id, serde_json::json!({"other": 1}), None).await)
        .await
        .expect("save");

    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let err = read_artifact_resource(&request, "srv", &repo)
        .await
        .unwrap_err();
    assert!(err.message.contains("no payload to render"));
}

#[tokio::test]
async fn read_artifact_renders_stored_payload_with_ui_meta() {
    let (db, repo) = repo().await;
    let id = fresh_id();
    let payload = serde_json::json!({
        "artifact": {
            "artifact_type": "message",
            "messages": [{"level": "info", "text": "stored artifact body"}]
        }
    });
    repo.save(
        &stored(
            &db,
            &id,
            payload,
            Some(
                ContextId::try_new("00000000-0000-4000-8000-0000000000ab")
                    .expect("valid ContextId"),
            ),
        )
        .await,
    )
    .await
    .expect("save");

    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let result = read_artifact_resource(&request, "srv", &repo)
        .await
        .expect("render");

    assert_eq!(result.contents.len(), 1);
    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("stored artifact body"));
    assert!(serialized.contains("text/html;profile=mcp-app"));
}

#[tokio::test]
async fn read_artifact_without_context_id_still_renders() {
    let (db, repo) = repo().await;
    let id = fresh_id();
    let payload = serde_json::json!({
        "artifact": {
            "artifact_type": "message",
            "messages": [{"level": "warning", "text": "context-free render"}]
        }
    });
    repo.save(&stored(&db, &id, payload, None).await)
        .await
        .expect("save");

    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let result = read_artifact_resource(&request, "srv", &repo)
        .await
        .expect("render");
    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("context-free render"));
}
