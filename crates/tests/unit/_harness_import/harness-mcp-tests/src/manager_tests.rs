use std::collections::BTreeMap;

use harness_mcp::{CancellationToken, McpError, RemoteHeaders, ServerStatus};
use serde_json::Value;

use crate::support::http::{HttpServer, Response, mcp_reply, tools};
use crate::support::manager::{local, manager, remote, settled, status_of};

#[tokio::test]
async fn disabled_servers_report_disabled_and_unknown_tools_fail() {
    let manager = manager();
    let mut config = local("does-not-matter", &[]);
    config.enabled = false;
    manager.reconcile(BTreeMap::from([("off".to_owned(), config)]));
    assert_eq!(manager.status().get("off"), Some(&ServerStatus::Disabled));
    assert_eq!(manager.tools(), Vec::<harness_mcp::McpTool>::new());
    let error = manager
        .call("off_tool", Value::Null, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::UnknownTool(_)));
    manager.reconcile(BTreeMap::new());
    assert!(manager.status().is_empty());
}

#[tokio::test]
async fn a_command_that_cannot_start_fails() {
    let manager = manager();
    let config = local("harness-mcp-test-no-such-binary", &[]);
    manager.reconcile(BTreeMap::from([("broken".to_owned(), config)]));
    settled(&manager).await;
    assert!(matches!(
        status_of(&manager, "broken"),
        ServerStatus::Failed { .. }
    ));
    assert!(matches!(
        manager.restart("missing"),
        Err(McpError::UnknownServer(_))
    ));
    manager.shutdown().await;
}

#[tokio::test]
async fn a_bad_url_fails_without_a_request() {
    let manager = manager();
    let config = remote("ftp://example.invalid".to_owned(), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("ftp".to_owned(), config)]));
    settled(&manager).await;
    assert!(matches!(
        status_of(&manager, "ftp"),
        ServerStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn a_remote_server_connects_and_serves_calls() {
    let listed = tools(&["echo"]);
    let server = HttpServer::start(move |request| mcp_reply(request, &listed)).await;
    let manager = manager();
    let config = remote(server.url("/mcp"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("r".to_owned(), config)]));
    settled(&manager).await;
    assert_eq!(
        status_of(&manager, "r"),
        ServerStatus::Connected { tools: 1 }
    );
    assert_eq!(
        manager.client("r").unwrap().server_info().unwrap().name,
        "fake"
    );
    let result = manager
        .call("r_echo", Value::Null, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.text, "called echo");
    let call = server.last_rpc("/mcp", "tools/call").unwrap();
    assert_eq!(call.header("mcp-session-id"), Some("session-1"));
    assert_eq!(call.header("mcp-protocol-version"), Some("2025-06-18"));
    manager.shutdown().await;
    assert!(manager.status().is_empty());
    assert!(manager.client("r").is_none());
}

#[tokio::test]
async fn a_server_that_answers_401_needs_auth() {
    let server = HttpServer::start(|_request| Response::status(401)).await;
    let manager = manager();
    let config = remote(server.url("/mcp"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("locked".to_owned(), config)]));
    settled(&manager).await;
    assert_eq!(status_of(&manager, "locked"), ServerStatus::NeedsAuth);
    assert_eq!(manager.tools(), Vec::<harness_mcp::McpTool>::new());
}

#[tokio::test]
async fn restart_reconnects_a_failed_server() {
    let listed = tools(&["echo"]);
    let server = HttpServer::start(move |request| {
        if request.rpc_method() == Some("initialize") && request.header("x-attempt").is_none() {
            return Response::status(500);
        }
        mcp_reply(request, &listed)
    })
    .await;
    let manager = manager();
    let config = remote(server.url("/mcp"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("flaky".to_owned(), config)]));
    settled(&manager).await;
    assert!(matches!(
        status_of(&manager, "flaky"),
        ServerStatus::Failed { .. }
    ));
    let retried = RemoteHeaders::Static(vec![("x-attempt".to_owned(), "2".to_owned())]);
    let config = remote(server.url("/mcp"), retried);
    manager.reconcile(BTreeMap::from([("flaky".to_owned(), config)]));
    settled(&manager).await;
    assert_eq!(
        status_of(&manager, "flaky"),
        ServerStatus::Connected { tools: 1 }
    );
    manager.restart("flaky").unwrap();
    settled(&manager).await;
    assert_eq!(server.rpc_count("/mcp", "initialize"), 3);
    assert_eq!(
        status_of(&manager, "flaky"),
        ServerStatus::Connected { tools: 1 }
    );
}
