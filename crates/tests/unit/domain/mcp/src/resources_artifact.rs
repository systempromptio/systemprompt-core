use rmcp::model::ReadResourceRequestParams;
use systemprompt_identifiers::{ArtifactId, ContextId, McpServerId, UserId};
use systemprompt_mcp::read_artifact_resource;
use systemprompt_mcp::repository::{CreateMcpArtifact, McpArtifactRepository};
use systemprompt_mcp::services::ui_renderer::registry::create_default_registry;
use systemprompt_test_fixtures::test_db_pool;

async fn repo() -> (systemprompt_database::DbPool, McpArtifactRepository) {
    let db = test_db_pool().await;
    let repo = McpArtifactRepository::new(&db);
    (db, repo)
}

const OWNER: &str = "11111111-1111-4111-8111-111111111abc";
const STRANGER: &str = "22222222-2222-4222-8222-222222222abc";

fn owner() -> UserId {
    UserId::new(OWNER)
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
    let mut create = CreateMcpArtifact::new(
        id.clone(),
        exec,
        McpServerId::new("res-tests"),
        "message",
        data,
    );
    create.context_id = context_id;
    create.user_id = Some(owner());
    create.title = Some("Stored Message".to_owned());
    create
}

#[tokio::test]
async fn read_artifact_rejects_non_artifact_uri() {
    let (_db, repo) = repo().await;
    let request = ReadResourceRequestParams::new("ui://srv/artifact-viewer");
    let err = read_artifact_resource(
        &request,
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("Not an artifact resource URI"));
}

#[tokio::test]
async fn read_artifact_rejects_server_mismatch() {
    let (_db, repo) = repo().await;
    let request = ReadResourceRequestParams::new("ui://other/artifact/abc");
    let err = read_artifact_resource(
        &request,
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
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
    let err = read_artifact_resource(
        &request,
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
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
    let err = read_artifact_resource(
        &request,
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
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
    let result = read_artifact_resource(
        &request,
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
    .await
    .expect("render");

    assert_eq!(result.contents.len(), 1);
    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("stored artifact body"));
    assert!(serialized.contains("text/html;profile=mcp-app"));
}

#[tokio::test]
async fn read_artifact_without_context_id_is_refused_instead_of_rendering_under_the_legacy_context()
{
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
    let error = read_artifact_resource(
        &request,
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
    .await
    .expect_err("an artifact with no context has nothing to render under");
    assert!(
        error.message.contains("has no context to render under"),
        "{error:?}"
    );
}

fn rendered_payload() -> serde_json::Value {
    serde_json::json!({
        "artifact": {
            "artifact_type": "message",
            "messages": [{"level": "info", "text": "owner only body"}]
        }
    })
}

fn render_context() -> ContextId {
    ContextId::try_new("00000000-0000-4000-8000-0000000000ac").expect("valid ContextId")
}

#[tokio::test]
async fn read_artifact_by_a_non_owner_is_indistinguishable_from_an_unknown_id() {
    let (db, repo) = repo().await;
    let id = fresh_id();
    repo.save(&stored(&db, &id, rendered_payload(), Some(render_context())).await)
        .await
        .expect("save");
    let missing = fresh_id();
    let registry = create_default_registry();
    let server = McpServerId::new("srv");

    let foreign = read_artifact_resource(
        &ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}")),
        &server,
        &repo,
        &registry,
        &UserId::new(STRANGER),
    )
    .await
    .expect_err("a non-owner cannot read the artifact");
    let unknown = read_artifact_resource(
        &ReadResourceRequestParams::new(format!("ui://srv/artifact/{missing}")),
        &server,
        &repo,
        &registry,
        &UserId::new(STRANGER),
    )
    .await
    .expect_err("an unknown id is refused");

    assert_eq!(foreign.code, unknown.code);
    assert_eq!(
        foreign.message.replace(id.as_str(), "<id>"),
        unknown.message.replace(missing.as_str(), "<id>")
    );
    assert_eq!(foreign.data, unknown.data);
    assert!(!foreign.message.contains("owner only body"));
}

#[tokio::test]
async fn read_artifact_by_its_owner_renders_the_resource() {
    let (db, repo) = repo().await;
    let id = fresh_id();
    repo.save(&stored(&db, &id, rendered_payload(), Some(render_context())).await)
        .await
        .expect("save");

    let result = read_artifact_resource(
        &ReadResourceRequestParams::new(format!("ui://srv/artifact/{id}")),
        &McpServerId::new("srv"),
        &repo,
        &create_default_registry(),
        &owner(),
    )
    .await
    .expect("the owner reads the artifact");

    let serialized = serde_json::to_string(&result.contents).expect("serializable");
    assert!(serialized.contains("owner only body"));
}
