use systemprompt_identifiers::McpServerId;
use systemprompt_runtime::documents::mcp::{DocumentsMcpTools, TOOL_NAMES, preview_resource_uri};

use crate::documents_mcp_support::{SERVER, harness};

const READ_ONLY: [&str; 6] = [
    "documents_list",
    "documents_search",
    "documents_read",
    "documents_history",
    "documents_download_url",
    "documents_render_preview",
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
async fn render_preview_points_at_the_document_preview_resource() {
    let h = harness().await;
    let tools = h.tools.list_tools();
    let preview = tools
        .iter()
        .find(|t| t.name == "documents_render_preview")
        .expect("preview tool");
    let meta = serde_json::to_string(&preview.meta).expect("meta json");
    let uri = preview_resource_uri(&McpServerId::new(SERVER));
    assert_eq!(uri, "ui://documents/document-preview");
    assert!(meta.contains(&uri), "meta: {meta}");
    let listed = h.tools.list_resources();
    assert_eq!(listed.resources.len(), 1);
    assert_eq!(listed.resources[0].uri, uri);
}
