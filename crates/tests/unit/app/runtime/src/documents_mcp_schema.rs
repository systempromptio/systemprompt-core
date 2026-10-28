use systemprompt_identifiers::McpServerId;
use systemprompt_runtime::documents::mcp::{DocumentsMcpTools, TOOL_NAMES, preview_resource_uri};

use crate::documents_mcp_support::{SERVER, harness};

const READ_ONLY: [&str; 8] = [
    "documents_list",
    "documents_search",
    "documents_read",
    "documents_history",
    "documents_access",
    "documents_export",
    "documents_guide",
    "documents_people",
];

#[tokio::test]
async fn tool_list_matches_the_plan_in_order() {
    let h = harness().await;
    let names: Vec<String> = h
        .tools
        .list_tools()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    assert_eq!(names, TOOL_NAMES.map(str::to_owned).to_vec());
    assert_eq!(DocumentsMcpTools::tool_names(), &TOOL_NAMES);
}

#[tokio::test]
async fn the_tool_list_stays_under_six_kilobytes() {
    let h = harness().await;
    let bytes = serde_json::to_vec(&h.tools.list_tools())
        .expect("tools json")
        .len();
    let per_tool: Vec<String> = h
        .tools
        .list_tools()
        .iter()
        .map(|tool| {
            format!(
                "{} in={} meta={} desc={}",
                tool.name,
                serde_json::to_vec(&tool.input_schema).map_or(0, |v| v.len()),
                serde_json::to_vec(&tool.meta).map_or(0, |v| v.len()),
                tool.description.as_ref().map_or(0, |d| d.len())
            )
        })
        .collect();
    assert!(bytes < 6144, "{bytes} bytes: {per_tool:#?}");
}

#[tokio::test]
async fn every_input_schema_is_an_object() {
    let h = harness().await;
    for tool in h.tools.list_tools() {
        assert_eq!(
            tool.input_schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{} inputSchema.type",
            tool.name
        );
    }
}

#[tokio::test]
async fn read_only_annotations_follow_the_plan() {
    let h = harness().await;
    for tool in h.tools.list_tools() {
        let read_only = tool
            .annotations
            .as_ref()
            .and_then(|a| a.read_only_hint)
            .unwrap_or(false);
        assert_eq!(
            read_only,
            READ_ONLY.contains(&tool.name.as_ref()),
            "{} read_only",
            tool.name
        );
    }
}

#[tokio::test]
async fn export_points_at_the_document_preview_resource() {
    let h = harness().await;
    let tools = h.tools.list_tools();
    let preview = tools
        .iter()
        .find(|t| t.name == "documents_export")
        .expect("preview tool");
    let meta = serde_json::to_string(&preview.meta).expect("meta json");
    let uri = preview_resource_uri(&McpServerId::new(SERVER));
    assert_eq!(uri, "ui://documents/document-preview");
    assert!(meta.contains(&uri), "meta: {meta}");
    let listed = h.tools.list_resources();
    assert_eq!(listed.resources.len(), 1);
    assert_eq!(listed.resources[0].uri, uri);
}

#[tokio::test]
async fn from_context_refuses_a_disabled_store_and_names_the_switch() {
    use systemprompt_runtime::documents::mcp::DocumentsMcpError;
    use systemprompt_test_fixtures::{
        ensure_test_bootstrap, fixture_app_context_documents_disabled,
        fixture_app_context_with_config, fixture_config, test_db_pool,
    };

    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let disabled =
        fixture_app_context_documents_disabled(&pool, fixture_config(&bootstrap.database_url))
            .expect("disabled context");
    let err = DocumentsMcpTools::from_context(&disabled, McpServerId::new(SERVER))
        .expect_err("a disabled store builds no tools");
    assert!(matches!(err, DocumentsMcpError::Disabled { .. }), "{err:?}");
    assert!(err.to_string().contains("files.documents.enabled"), "{err}");

    let enabled = fixture_app_context_with_config(&pool, fixture_config(&bootstrap.database_url))
        .expect("enabled context");
    let tools = DocumentsMcpTools::from_context(&enabled, McpServerId::new(SERVER))
        .expect("an enabled store builds the tools");
    assert_eq!(tools.server_name().as_str(), SERVER);
}
