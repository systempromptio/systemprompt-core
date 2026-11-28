//! One server session: paged listings, tolerant tool lists, call results,
//! resources, prompts and per-request timeouts.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Value, json};
use systemprompt_engine::mcp::{McpError, RequestOptions};

use crate::support::{connected, drain, initialize, method, reply};

fn quick() -> RequestOptions {
    RequestOptions::with_timeout(Duration::from_millis(300))
}

#[tokio::test]
async fn prompts_listed_over_several_pages_can_be_rendered() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"prompts": {}})),
        "prompts/list" => match m["params"]["cursor"].as_str() {
            None => vec![reply(m, &json!({"prompts": [{"name": "review", "arguments": [{"name": "file", "required": true}]}], "nextCursor": "2"}))],
            Some(_) => vec![reply(m, &json!({"prompts": [{"name": "explain"}]}))],
        },
        "prompts/get" => vec![reply(m, &json!({
            "description": "a review",
            "messages": [{"role": "user", "content": {"type": "text", "text": format!("review {}", m["params"]["arguments"]["file"].as_str().unwrap_or_default())}}]
        }))],
        _ => Vec::new(),
    }))
    .await;
    let prompts = client
        .list_prompts(RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(
        prompts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["review", "explain"]
    );
    assert!(prompts[0].arguments[0].required);
    let arguments = BTreeMap::from([("file".to_owned(), "main.rs".to_owned())]);
    let rendered = client
        .get_prompt("review", &arguments, RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(rendered.messages[0].text, "review main.rs");
    client.shutdown().await;
}

#[tokio::test]
async fn a_tool_whose_output_schema_is_not_an_object_is_still_listed() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "tools/list" => vec![reply(m, &json!({"tools": [{"name": "legacy", "inputSchema": {"type": "object"}, "outputSchema": {"type": "array"}}]}))],
        _ => Vec::new(),
    }))
    .await;
    let tools = client.list_tools(RequestOptions::default()).await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "legacy");
    client.shutdown().await;
}

#[tokio::test]
async fn unusable_tools_are_skipped_and_the_rest_of_the_list_is_kept() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "tools/list" => vec![reply(m, &json!({"tools": [
            {"name": "good"},
            {"description": "no name"},
            {"name": 42},
            {"name": "also_good", "inputSchema": {"type": "object", "properties": {"x": {"type": "string"}}}}
        ]}))],
        _ => Vec::new(),
    }))
    .await;
    let names: Vec<String> = client
        .list_tools(RequestOptions::default())
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["good", "also_good"]);
    client.shutdown().await;
}

#[tokio::test]
async fn a_result_that_does_not_match_the_declared_output_schema_is_returned_as_sent() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "tools/call" => vec![reply(
            m,
            &json!({"content": [], "structuredContent": {"unexpected": [1, 2]}}),
        )],
        _ => Vec::new(),
    }))
    .await;
    let result = client
        .call_tool("t", json!({}), RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(result.structured, Some(json!({"unexpected": [1, 2]})));
    assert!(!result.is_error);
    client.shutdown().await;
}

#[tokio::test]
async fn a_call_the_server_flags_as_an_error_is_reported_as_failed() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "tools/call" => vec![reply(
            m,
            &json!({"content": [{"type": "text", "text": "disk full"}], "isError": true}),
        )],
        _ => Vec::new(),
    }))
    .await;
    let result = client
        .call_tool("write", json!({}), RequestOptions::default())
        .await
        .unwrap();
    assert!(result.is_error);
    assert_eq!(result.text, "disk full");
    client.shutdown().await;
}

#[tokio::test]
#[ignore = "parity ledger: JSON text from a tool without an output schema is kept as text and not parsed into a structured value"]
async fn json_text_from_a_tool_without_an_output_schema_is_parsed() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "tools/call" => vec![reply(
            m,
            &json!({"content": [{"type": "text", "text": "{\"count\": 3}"}]}),
        )],
        _ => Vec::new(),
    }))
    .await;
    let result = client
        .call_tool("count", json!({}), RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(result.structured, Some(json!({"count": 3})));
}

#[tokio::test]
#[ignore = "parity ledger: a text blob in an embedded resource is shown as a resource link instead of being decoded to its text"]
async fn a_text_blob_in_a_tool_result_is_decoded_to_text() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "tools/call" => vec![reply(m, &json!({"content": [{"type": "resource", "resource": {"uri": "file:///notes.txt", "mimeType": "text/plain", "blob": "aGVsbG8gd29ybGQ="}}]}))],
        _ => Vec::new(),
    }))
    .await;
    let result = client
        .call_tool("get", json!({}), RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(result.text, "hello world");
}

#[tokio::test]
async fn a_tool_call_that_outlives_its_timeout_fails_with_a_timeout() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        _ => Vec::new(),
    }))
    .await;
    let error = client
        .call_tool("slow", json!({}), quick())
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::Timeout(_)), "{error:?}");
    client.shutdown().await;
}

#[tokio::test]
async fn listing_prompts_resources_and_tools_and_reading_a_resource_honour_their_timeouts() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}, "prompts": {}, "resources": {}})),
        _ => Vec::new(),
    }))
    .await;
    assert!(matches!(
        client.list_tools(quick()).await,
        Err(McpError::Timeout(_))
    ));
    assert!(matches!(
        client.list_prompts(quick()).await,
        Err(McpError::Timeout(_))
    ));
    assert!(matches!(
        client.get_prompt("p", &BTreeMap::new(), quick()).await,
        Err(McpError::Timeout(_))
    ));
    assert!(matches!(
        client.list_resources(quick()).await,
        Err(McpError::Timeout(_))
    ));
    assert!(matches!(
        client.read_resource("file:///a", quick()).await,
        Err(McpError::Timeout(_))
    ));
    client.shutdown().await;
}

#[tokio::test]
#[ignore = "parity ledger: resources are requested from a server that did not advertise the resources capability"]
async fn resources_are_not_requested_from_a_server_without_the_capability() {
    let (client, mut seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"tools": {}})),
        "resources/list" => vec![reply(m, &json!({"resources": []}))],
        _ => Vec::new(),
    }))
    .await;
    let listed = client
        .list_resources(RequestOptions::default())
        .await
        .unwrap();
    assert!(listed.is_empty());
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!drain(&mut seen).iter().any(|m| m == "resources/list"));
}

#[tokio::test]
async fn a_resource_catalogue_decodes_without_inventing_absent_metadata() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"resources": {}})),
        "resources/list" => vec![reply(m, &json!({"resources": [
            {"uri": "file:///bare"},
            {"uri": "file:///full", "name": "full", "title": "Full", "description": "all fields", "mimeType": "text/markdown"}
        ]}))],
        _ => Vec::new(),
    }))
    .await;
    let resources = client
        .list_resources(RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(resources[0].uri, "file:///bare");
    assert_eq!(
        (
            resources[0].title.clone(),
            resources[0].description.clone(),
            resources[0].mime_type.clone()
        ),
        (None, None, None)
    );
    assert_eq!(resources[1].mime_type.as_deref(), Some("text/markdown"));
    client.shutdown().await;
}

#[tokio::test]
async fn resource_contents_keep_text_and_base64_binary_items() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({"resources": {}})),
        "resources/read" => vec![reply(
            m,
            &json!({"contents": [
                {"uri": "file:///a.txt", "mimeType": "text/plain", "text": "hello"},
                {"uri": "file:///b.bin", "mimeType": "application/octet-stream", "blob": "AAECAw=="}
            ]}),
        )],
        _ => Vec::new(),
    }))
    .await;
    let contents = client
        .read_resource("file:///", RequestOptions::default())
        .await
        .unwrap();
    assert_eq!(contents[0].text.as_deref(), Some("hello"));
    assert_eq!(contents[1].blob.as_deref(), Some("AAECAw=="));
    assert!(contents[1].text.is_none());
    client.shutdown().await;
}

#[tokio::test]
async fn concurrent_closes_of_one_session_both_complete() {
    let (client, _seen) = connected(Box::new(|m| match method(m) {
        "initialize" => initialize(m, &json!({})),
        _ => Vec::new(),
    }))
    .await;
    let other = client.clone();
    let both = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(client.shutdown(), other.shutdown())
    })
    .await;
    assert!(both.is_ok());
    assert!(client.is_closed());
    let after: Result<Value, _> = client
        .request("ping", None, RequestOptions::default())
        .await;
    assert!(after.is_err());
}
