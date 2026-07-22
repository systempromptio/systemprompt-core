use rmcp::model::ReadResourceRequestParams;
use systemprompt_identifiers::{ArtifactId, ContextId, McpExecutionId};
use systemprompt_mcp::read_artifact_resource;
use systemprompt_mcp::repository::{CreateMcpArtifact, McpArtifactRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn repo() -> Option<McpArtifactRepository> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    McpArtifactRepository::new(&db).ok()
}

fn fresh_id() -> ArtifactId {
    ArtifactId::new(format!("art-{}", uuid::Uuid::new_v4().simple()))
}

fn stored(id: &ArtifactId, data: serde_json::Value, context_id: Option<ContextId>) -> CreateMcpArtifact {
    CreateMcpArtifact {
        artifact_id: id.clone(),
        mcp_execution_id: McpExecutionId::new(format!("exec-{}", uuid::Uuid::new_v4().simple())),
        context_id,
        user_id: None,
        server_name: "res-tests".to_owned(),
        artifact_type: "message".to_owned(),
        title: Some("Stored Message".to_owned()),
        data,
        metadata: None,
        expires_at: None,
    }
}

#[tokio::test]
async fn read_artifact_rejects_non_artifact_uri() {
    let Some(repo) = repo().await else { return };
    let request = ReadResourceRequestParams::new("ui://srv/artifact-viewer");
    let err = read_artifact_resource(&request, "srv", &repo).await.unwrap_err();
    assert!(err.message.contains("Not an artifact resource URI"));
}

#[tokio::test]
async fn read_artifact_rejects_server_mismatch() {
    let Some(repo) = repo().await else { return };
    let request = ReadResourceRequestParams::new("ui://other/artifact/abc");
    let err = read_artifact_resource(&request, "srv", &repo).await.unwrap_err();
    assert!(err.message.contains("names server 'other'"));
    assert!(err.message.contains("'srv'"));
}

#[tokio::test]
async fn read_artifact_unknown_id_is_invalid_params() {
    let Some(repo) = repo().await else { return };
    let id = fresh_id();
    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let err = read_artifact_resource(&request, "srv", &repo).await.unwrap_err();
    assert!(err.message.contains("Unknown artifact"));
}

#[tokio::test]
async fn read_artifact_without_payload_key_is_internal_error() {
    let Some(repo) = repo().await else { return };
    let id = fresh_id();
    repo.save(&stored(&id, serde_json::json!({"other": 1}), None))
        .await
        .expect("save");

    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let err = read_artifact_resource(&request, "srv", &repo).await.unwrap_err();
    assert!(err.message.contains("no payload to render"));
}

#[tokio::test]
async fn read_artifact_renders_stored_payload_with_ui_meta() {
    let Some(repo) = repo().await else { return };
    let id = fresh_id();
    let payload = serde_json::json!({
        "artifact": {
            "artifact_type": "message",
            "messages": [{"level": "info", "text": "stored artifact body"}]
        }
    });
    repo.save(&stored(
        &id,
        payload,
        Some(ContextId::new("00000000-0000-4000-8000-0000000000ab")),
    ))
    .await
    .expect("save");

    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let result = read_artifact_resource(&request, "srv", &repo).await.expect("render");

    assert_eq!(result.contents.len(), 1);
    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("stored artifact body"));
    assert!(serialized.contains("text/html;profile=mcp-app"));
}

#[tokio::test]
async fn read_artifact_without_context_id_still_renders() {
    let Some(repo) = repo().await else { return };
    let id = fresh_id();
    let payload = serde_json::json!({
        "artifact": {
            "artifact_type": "message",
            "messages": [{"level": "warning", "text": "context-free render"}]
        }
    });
    repo.save(&stored(&id, payload, None)).await.expect("save");

    let request = ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}"));
    let result = read_artifact_resource(&request, "srv", &repo).await.expect("render");
    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("context-free render"));
}
