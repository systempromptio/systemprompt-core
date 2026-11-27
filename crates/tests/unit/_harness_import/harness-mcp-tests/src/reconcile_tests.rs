use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use harness_mcp::{BoxError, CancellationToken, HeaderSource, RemoteHeaders, ServerStatus};
use serde_json::Value;

use crate::support::http::{HttpServer, Response, mcp_reply, tools};
use crate::support::manager::{bearer, manager, remote, settled, status_of};

struct Token(&'static str);

#[async_trait::async_trait]
impl HeaderSource for Token {
    async fn headers(&self) -> Result<Vec<(String, String)>, BoxError> {
        Ok(vec![(
            "authorization".to_owned(),
            format!("Bearer {}", self.0),
        )])
    }
}

async fn echo_server() -> HttpServer {
    let listed = tools(&["echo"]);
    HttpServer::start(move |request| mcp_reply(request, &listed)).await
}

#[tokio::test]
async fn adds_removes_keeps_and_parks() {
    let server = echo_server().await;
    let manager = manager();
    let keep = remote(server.url("/keep"), RemoteHeaders::default());
    let gone = remote(server.url("/gone"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([
        ("keep".to_owned(), keep.clone()),
        ("gone".to_owned(), gone),
    ]));
    settled(&manager).await;
    let mut parked = remote(server.url("/parked"), RemoteHeaders::default());
    parked.enabled = false;
    let added = remote(server.url("/new"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([
        ("keep".to_owned(), keep),
        ("new".to_owned(), added),
        ("parked".to_owned(), parked),
    ]));
    settled(&manager).await;
    let names: Vec<String> = manager.status().into_keys().collect();
    assert_eq!(names, ["keep", "new", "parked"]);
    assert_eq!(server.rpc_count("/keep", "initialize"), 1);
    assert_eq!(server.rpc_count("/new", "initialize"), 1);
    assert_eq!(server.rpc_count("/parked", "initialize"), 0);
    assert_eq!(status_of(&manager, "parked"), ServerStatus::Disabled);
    let tools: Vec<String> = manager
        .tools()
        .into_iter()
        .map(|tool| tool.qualified_name)
        .collect();
    assert_eq!(tools, ["keep_echo", "new_echo"]);
    manager.shutdown().await;
}

#[tokio::test]
async fn an_endpoint_change_restarts() {
    let server = echo_server().await;
    let manager = manager();
    let first = remote(server.url("/a"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("r".to_owned(), first)]));
    settled(&manager).await;
    let moved = remote(server.url("/b"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("r".to_owned(), moved)]));
    settled(&manager).await;
    assert_eq!(server.rpc_count("/b", "initialize"), 1);
    assert_eq!(
        status_of(&manager, "r"),
        ServerStatus::Connected { tools: 1 }
    );
    manager.shutdown().await;
}

#[tokio::test]
async fn a_header_change_swaps_in_place_on_a_live_server() {
    let server = echo_server().await;
    let manager = manager();
    manager.reconcile(BTreeMap::from([(
        "live".to_owned(),
        remote(server.url("/live"), bearer("1")),
    )]));
    settled(&manager).await;
    manager.reconcile(BTreeMap::from([(
        "live".to_owned(),
        remote(server.url("/live"), bearer("2")),
    )]));
    settled(&manager).await;
    manager
        .call("live_echo", Value::Null, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(server.rpc_count("/live", "initialize"), 1);
    let call = server.last_rpc("/live", "tools/call").unwrap();
    assert_eq!(call.header("authorization"), Some("Bearer 2"));
    manager.shutdown().await;
}

#[tokio::test]
async fn a_header_change_restarts_a_server_that_needed_auth() {
    let listed = tools(&["echo"]);
    let server = HttpServer::start(move |request| {
        if request.header("authorization") == Some("Bearer 2") {
            return mcp_reply(request, &listed);
        }
        Response::status(401)
    })
    .await;
    let manager = manager();
    manager.reconcile(BTreeMap::from([(
        "auth".to_owned(),
        remote(server.url("/auth"), bearer("1")),
    )]));
    settled(&manager).await;
    assert_eq!(status_of(&manager, "auth"), ServerStatus::NeedsAuth);
    manager.reconcile(BTreeMap::from([(
        "auth".to_owned(),
        remote(server.url("/auth"), bearer("2")),
    )]));
    settled(&manager).await;
    assert_eq!(
        status_of(&manager, "auth"),
        ServerStatus::Connected { tools: 1 }
    );
    assert_eq!(server.rpc_count("/auth", "initialize"), 2);
    manager.shutdown().await;
}

#[tokio::test]
async fn the_same_source_is_unchanged_and_a_new_one_swaps() {
    let server = echo_server().await;
    let manager = manager();
    let source: Arc<dyn HeaderSource> = Arc::new(Token("old"));
    let config =
        |source: Arc<dyn HeaderSource>| remote(server.url("/s"), RemoteHeaders::Source(source));
    manager.reconcile(BTreeMap::from([(
        "s".to_owned(),
        config(Arc::clone(&source)),
    )]));
    settled(&manager).await;
    manager.reconcile(BTreeMap::from([("s".to_owned(), config(source))]));
    let fresh: Arc<dyn HeaderSource> = Arc::new(Token("new"));
    manager.reconcile(BTreeMap::from([("s".to_owned(), config(fresh))]));
    settled(&manager).await;
    manager
        .call("s_echo", Value::Null, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(server.rpc_count("/s", "initialize"), 1);
    let call = server.last_rpc("/s", "tools/call").unwrap();
    assert_eq!(call.header("authorization"), Some("Bearer new"));
    manager.shutdown().await;
}

#[tokio::test]
async fn disabling_and_enabling() {
    let server = echo_server().await;
    let manager = manager();
    let on = remote(server.url("/t"), RemoteHeaders::default());
    let mut off = on.clone();
    off.enabled = false;
    manager.reconcile(BTreeMap::from([("t".to_owned(), on.clone())]));
    settled(&manager).await;
    manager.reconcile(BTreeMap::from([("t".to_owned(), off)]));
    assert_eq!(status_of(&manager, "t"), ServerStatus::Disabled);
    assert_eq!(manager.tools(), Vec::<harness_mcp::McpTool>::new());
    assert!(manager.client("t").is_none());
    manager.reconcile(BTreeMap::from([("t".to_owned(), on)]));
    settled(&manager).await;
    assert_eq!(
        status_of(&manager, "t"),
        ServerStatus::Connected { tools: 1 }
    );
    assert_eq!(server.rpc_count("/t", "initialize"), 2);
    manager.shutdown().await;
}

#[tokio::test]
async fn a_timeout_only_change_does_not_restart() {
    let server = echo_server().await;
    let manager = manager();
    let config = remote(server.url("/t"), RemoteHeaders::default());
    let mut slower = config.clone();
    slower.connect_timeout = Some(Duration::from_secs(90));
    manager.reconcile(BTreeMap::from([("t".to_owned(), config)]));
    settled(&manager).await;
    manager.reconcile(BTreeMap::from([("t".to_owned(), slower)]));
    settled(&manager).await;
    assert_eq!(server.rpc_count("/t", "initialize"), 1);
    assert_eq!(
        status_of(&manager, "t"),
        ServerStatus::Connected { tools: 1 }
    );
    manager.shutdown().await;
}
