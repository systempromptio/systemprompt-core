//! DB-backed tests for the UI resource `McpResponseBuilder::build` attaches.
//!
//! This is the exact path an MCP tool call takes: a `CliArtifact` in, a
//! `CallToolResult` out carrying server-rendered HTML for the host to mount.

use rmcp::model::{CallToolResult, ResourceContents};
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::{ArtifactIngest, ClientProfile, McpResponseBuilder, ToolIdentity};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{
    CardSection, CliArtifact, Column, ColumnType, PresentationCardArtifact, TableArtifact,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn ui_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(rmcp::model::ProtocolVersion::V_2025_06_18),
        client_name: Some("test-host".to_owned()),
        extensions: [systemprompt_models::mcp::EXTENSION_ID.to_owned()].into(),
    }
}

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("s-{}", uuid::Uuid::new_v4().simple())),
        TraceId::new("t"),
        ContextId::generate(),
        AgentName::try_new("a").expect("valid AgentName"),
    )
}

async fn build(artifact: CliArtifact, repo: &ArtifactIngest) -> CallToolResult {
    let context = ctx();
    let exec_id = McpExecutionId::new(format!("exec-{}", uuid::Uuid::new_v4().simple()));

    McpResponseBuilder::new(
        artifact,
        ToolIdentity::new("systemprompt", "cli_execute"),
        &context,
        &exec_id,
        &ui_client(),
    )
    .build("summary", repo, "cli", Some("User Directory".to_owned()))
    .await
    .expect("response builds")
}

fn ui_resource(result: &CallToolResult) -> (String, String) {
    for block in &result.content {
        if let Some(resource) = block.as_resource()
            && let ResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } = &resource.resource
        {
            assert_eq!(
                mime_type.as_deref(),
                Some("text/html;profile=mcp-app"),
                "embedded artifact resource must be an MCP app"
            );
            return (uri.clone(), text.clone());
        }
    }
    panic!("tool result carries no embedded ui:// resource");
}

#[tokio::test]
async fn table_tool_result_embeds_rendered_table_html() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ArtifactIngest::from_db(&db, None).expect("artifact ingest");

    let table = TableArtifact::new(vec![Column::new("email", ColumnType::String)])
        .with_rows(vec![serde_json::json!({"email": "ed@example.com"})]);
    let result = build(CliArtifact::table(table), &repo).await;

    let (uri, html) = ui_resource(&result);

    assert!(
        uri.starts_with("ui://systemprompt/artifact/"),
        "unexpected resource uri: {uri}"
    );
    assert!(html.contains("data-table"));
    assert!(html.contains("ed@example.com"));
    assert!(
        !html.contains("UNKNOWN"),
        "rendered artifact must not fall back to an unknown type"
    );
    assert!(
        html.contains("ui/notifications/size-changed"),
        "rendered artifact must negotiate its height with the host"
    );
}

#[tokio::test]
async fn presentation_card_tool_result_embeds_rendered_card_html() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ArtifactIngest::from_db(&db, None).expect("artifact ingest");

    let card = PresentationCardArtifact::new("Platform Overview")
        .with_sections(vec![CardSection::new("Total users", "15")]);
    let result = build(CliArtifact::presentation_card(card), &repo).await;

    let (_uri, html) = ui_resource(&result);

    assert!(html.contains("Platform Overview"));
    assert!(html.contains("Total users"));
    assert!(html.contains("card-section"));
}

// MCP Apps sends dimensions as {width, height}; height alone is not the shape.
#[tokio::test]
async fn rendered_artifact_reports_both_dimensions_to_the_host() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ArtifactIngest::from_db(&db, None).expect("artifact ingest");

    let table = TableArtifact::new(vec![Column::new("id", ColumnType::String)]);
    let result = build(CliArtifact::table(table), &repo).await;
    let (_uri, html) = ui_resource(&result);

    assert!(html.contains("ui/notifications/size-changed"));
    assert!(html.contains("params: { width, height }"));
}

// The shell needs this URI to fall back to resources/read when a host does
// not forward embedded content blocks.
#[tokio::test]
async fn result_meta_names_the_ui_resource_uri() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ArtifactIngest::from_db(&db, None).expect("artifact ingest");

    let table = TableArtifact::new(vec![Column::new("id", ColumnType::String)]);
    let result = build(CliArtifact::table(table), &repo).await;

    let meta = result.meta.as_ref().expect("result carries _meta");
    let uri = meta
        .get(systemprompt_mcp::UI_RESOURCE_URI_META_KEY)
        .and_then(|v| v.as_str())
        .expect("_meta names the ui resource uri");

    let (embedded_uri, _html) = ui_resource(&result);
    assert_eq!(uri, embedded_uri, "_meta uri must match the embedded block");
}

#[tokio::test]
async fn structured_content_still_accompanies_the_rendered_resource() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ArtifactIngest::from_db(&db, None).expect("artifact ingest");

    let table = TableArtifact::new(vec![Column::new("id", ColumnType::String)]);
    let result = build(CliArtifact::table(table), &repo).await;

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content preserved");
    assert_eq!(
        structured.get("artifact_type").and_then(|t| t.as_str()),
        Some("table")
    );
}
#[tokio::test]
async fn response_build_failure_leaves_no_artifact_and_same_database_recovers() {
    let database = systemprompt_test_fixtures::DisposableDb::installed("mcp_response_recovery")
        .await
        .expect("private response database");
    let failed_db = database.pool().await.expect("failure pool");
    let failed_repo = ArtifactIngest::from_db(&failed_db, None).expect("artifact ingest");
    failed_db
        .write_pool_arc()
        .expect("write pool")
        .close()
        .await;
    let context = ctx();
    let exec_id = McpExecutionId::new(format!("exec-{}", uuid::Uuid::new_v4().simple()));
    let table = || {
        CliArtifact::table(
            TableArtifact::new(vec![Column::new("marker", ColumnType::String)])
                .with_rows(vec![serde_json::json!({"marker": "durable-payload"})]),
        )
    };

    let error = McpResponseBuilder::new(
        table(),
        ToolIdentity::new("recovery-server", "recovery-tool"),
        &context,
        &exec_id,
        &ui_client(),
    )
    .build(
        "recovery summary",
        &failed_repo,
        "cli",
        Some("Recovery table".to_owned()),
    )
    .await
    .expect_err("closed persistence must fail the response build");
    assert!(
        error.to_string().contains("Failed to persist artifact"),
        "{error}"
    );
    drop(failed_repo);
    drop(failed_db);

    let recovered_db = database.pool().await.expect("reconnected pool");
    let recovered_repo = ArtifactIngest::from_db(&recovered_db, None).expect("recovered ingest");
    assert!(
        recovered_repo
            .artifacts()
            .find_by_execution_id(&exec_id)
            .await
            .expect("failed-attempt lookup")
            .is_none(),
        "the failed response build must not leave an artifact row"
    );

    McpResponseBuilder::new(
        table(),
        ToolIdentity::new("recovery-server", "recovery-tool"),
        &context,
        &exec_id,
        &ui_client(),
    )
    .build(
        "recovery summary",
        &recovered_repo,
        "cli",
        Some("Recovery table".to_owned()),
    )
    .await
    .expect("the same database accepts the retry after reconnecting");
    let stored = recovered_repo
        .artifacts()
        .find_by_execution_id(&exec_id)
        .await
        .expect("successful-attempt lookup")
        .expect("successful retry persists an artifact");
    assert_eq!(stored.artifact_type, "table");
    let stored_response: systemprompt_models::artifacts::ToolResponse<TableArtifact> =
        serde_json::from_value(stored.data.clone()).expect("stored tool-response envelope");
    assert_eq!(stored_response.artifact_id, stored.artifact_id);
    assert_eq!(stored_response.mcp_execution_id, exec_id);
    assert_eq!(stored_response.artifact.artifact_type, "table");
    assert_eq!(
        stored_response.artifact.items,
        vec![serde_json::json!({"marker": "durable-payload"})]
    );
    assert_eq!(stored_response.artifact.columns.len(), 1);
    assert_eq!(stored_response.artifact.columns[0].name, "marker");
    assert_eq!(stored.server_name, "recovery-server");
    assert_eq!(stored.tool_name.as_deref(), Some("recovery-tool"));

    recovered_db
        .write_pool_arc()
        .expect("write pool")
        .close()
        .await;
    drop(recovered_repo);
    drop(recovered_db);
    database.drop_now().await;
}
