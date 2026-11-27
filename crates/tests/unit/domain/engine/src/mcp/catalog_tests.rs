use std::collections::BTreeMap;

use serde_json::{Value, json};
use systemprompt_engine::mcp::{
    CancellationToken, MAX_QUALIFIED_NAME_LEN, McpError, RemoteHeaders, qualify,
};

use crate::mcp::support::http::{HttpServer, mcp_reply};
use crate::mcp::support::manager::{manager, remote, settled};

#[test]
fn qualify_sanitises_and_truncates() {
    assert_eq!(qualify("git hub", "create.issue"), "git_hub_create_issue");
    assert_eq!(qualify("drive", "list-files"), "drive_list-files");
    assert_eq!(qualify("sérver", "tööl"), "s_rver_t__l");
    let long = qualify(&"s".repeat(40), &"t".repeat(40));
    assert_eq!(long.len(), MAX_QUALIFIED_NAME_LEN);
    assert!(long.starts_with(&"s".repeat(40)));
}

#[tokio::test]
async fn colliding_names_get_suffixes_within_the_limit() {
    let long = "t".repeat(70);
    let longer = format!("{long}x");
    let listed = json!([{"name": "a.b"}, {"name": "a_b"}, {"name": "a b"}, {"name": long}, {"name": longer}]);
    let server = HttpServer::start(move |request| mcp_reply(request, &listed)).await;
    let manager = manager();
    let config = remote(server.url("/mcp"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("x".to_owned(), config)]));
    settled(&manager).await;
    let names: Vec<String> = manager
        .tools()
        .iter()
        .map(|tool| tool.qualified_name.clone())
        .collect();
    assert_eq!(names[..3], ["x_a_b", "x_a_b_2", "x_a_b_3"]);
    assert_eq!(names[3].len(), MAX_QUALIFIED_NAME_LEN);
    assert_eq!(names[4].len(), MAX_QUALIFIED_NAME_LEN);
    assert!(names[4].ends_with("_2"));
    let result = manager
        .call(&names[4], Value::Null, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.text, format!("called {longer}"));
    manager.shutdown().await;
}

#[tokio::test]
async fn schemas_become_objects_and_titles_stand_in_for_descriptions() {
    let listed = json!([
        {"name": "t", "title": "Title only", "inputSchema": {"properties": {}}},
        {"name": "u", "description": "described", "inputSchema": "not a schema"},
    ]);
    let server = HttpServer::start(move |request| mcp_reply(request, &listed)).await;
    let manager = manager();
    let config = remote(server.url("/mcp"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("s".to_owned(), config)]));
    settled(&manager).await;
    let tools = manager.tools();
    assert_eq!(tools[0].qualified_name, "s_t");
    assert_eq!(tools[0].server, "s");
    assert_eq!(
        tools[0].input_schema,
        json!({"properties": {}, "type": "object"})
    );
    assert_eq!(tools[0].description, "Title only");
    assert_eq!(
        tools[1].input_schema,
        json!({"type": "object", "properties": {}})
    );
    assert_eq!(tools[1].description, "described");
    let error = manager
        .call("s_missing", Value::Null, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::UnknownTool { .. }));
    manager.shutdown().await;
}
