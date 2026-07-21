//! DB-backed tests for the UI resource `McpResponseBuilder::build` attaches.
//!
//! This is the exact path an MCP tool call takes: a `CliArtifact` in, a
//! `CallToolResult` out carrying server-rendered HTML for the host to mount.

use rmcp::model::{CallToolResult, ResourceContents};
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId};
use systemprompt_mcp::McpResponseBuilder;
use systemprompt_mcp::repository::McpArtifactRepository;
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{
    CardSection, CliArtifact, Column, ColumnType, PresentationCardArtifact, TableArtifact,
};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("s-{}", uuid::Uuid::new_v4().simple())),
        TraceId::new("t"),
        ContextId::generate(),
        AgentName::new("a"),
    )
}

async fn build(artifact: CliArtifact, repo: &McpArtifactRepository) -> CallToolResult {
    let context = ctx();
    let exec_id = McpExecutionId::new(format!("exec-{}", uuid::Uuid::new_v4().simple()));

    McpResponseBuilder::new(artifact, "systemprompt", &context, &exec_id)
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
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).expect("repo");

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
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).expect("repo");

    let card = PresentationCardArtifact::new("Platform Overview")
        .with_sections(vec![CardSection::new("Total users", "15")]);
    let result = build(CliArtifact::presentation_card(card), &repo).await;

    let (_uri, html) = ui_resource(&result);

    assert!(html.contains("Platform Overview"));
    assert!(html.contains("Total users"));
    assert!(html.contains("card-section"));
}

/// MCP Apps (SEP-1865) sends the app its dimensions as `{width, height}`;
/// a height-only notification does not satisfy the schema.
#[tokio::test]
async fn rendered_artifact_reports_both_dimensions_to_the_host() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).expect("repo");

    let table = TableArtifact::new(vec![Column::new("id", ColumnType::String)]);
    let result = build(CliArtifact::table(table), &repo).await;
    let (_uri, html) = ui_resource(&result);

    assert!(html.contains("ui/notifications/size-changed"));
    assert!(html.contains("params: { width, height }"));
}

/// The app shell needs the artifact's own `ui://` URI to fall back to
/// `resources/read` when a host does not forward embedded content blocks.
#[tokio::test]
async fn result_meta_names_the_ui_resource_uri() {
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).expect("repo");

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
    let Some(db) = db().await else { return };
    let repo = McpArtifactRepository::new(&db).expect("repo");

    let table = TableArtifact::new(vec![Column::new("id", ColumnType::String)]);
    let result = build(CliArtifact::table(table), &repo).await;

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content preserved");
    assert_eq!(
        structured
            .get("artifact")
            .and_then(|a| a.get("artifact_type"))
            .and_then(|t| t.as_str()),
        Some("table")
    );
}
