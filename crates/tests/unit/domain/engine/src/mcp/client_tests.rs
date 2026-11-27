use std::time::Duration;

use serde_json::{Value, json};
use systemprompt_engine::mcp::{
    CancellationToken, ClientInfo, LATEST_PROTOCOL_VERSION, McpClient, McpError, RequestOptions,
};

use crate::mcp::support::stream::{
    Handler, connected, fake_server, initialize_result, next_with_method, reply, rpc_error,
    standard,
};

#[tokio::test]
async fn initialize_negotiates_and_announces_itself() {
    let (client, mut seen) = connected(Box::new(standard)).await;
    let init = next_with_method(&mut seen, "initialize").await;
    assert_eq!(
        init["params"]["clientInfo"],
        json!({"name": "tester", "version": "1.0"})
    );
    assert_eq!(init["params"]["protocolVersion"], LATEST_PROTOCOL_VERSION);
    let initialized = next_with_method(&mut seen, "notifications/initialized").await;
    assert!(initialized.get("id").is_none());
    assert!(initialized.get("params").is_none());
    let info = client.server_info().unwrap();
    assert_eq!(info.name, "fake");
    assert_eq!(info.protocol_version, "2025-06-18");
    assert_eq!(info.instructions.as_deref(), Some("use the fake tools"));
    assert!(info.has_capability("tools"));
    assert!(!info.has_capability("prompts"));
}

#[tokio::test]
async fn initialize_rejects_an_unknown_protocol_version() {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let handler: Handler = Box::new(|message: &Value| {
        if message["method"] == "initialize" {
            return vec![reply(message, &initialize_result("1999-01-01"))];
        }
        Vec::new()
    });
    let _seen = fake_server(server_io, handler);
    let (read, write) = tokio::io::split(client_io);
    let client = McpClient::from_stream(read, write);
    let error = client
        .initialize(&ClientInfo::new("tester", "1.0"), None)
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::ProtocolVersion { ref version } if version == "1999-01-01"));
    assert!(client.server_info().is_none());
}

#[tokio::test]
async fn list_tools_follows_pages_and_omits_absent_params() {
    let (client, mut seen) = connected(Box::new(standard)).await;
    let tools = client.list_tools(RequestOptions::default()).await.unwrap();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["echo", "add"]);
    assert_eq!(tools[1].description.as_deref(), Some("adds"));
    let first = next_with_method(&mut seen, "tools/list").await;
    assert!(first.get("params").is_none());
    assert_eq!(first["jsonrpc"], "2.0");
    let second = next_with_method(&mut seen, "tools/list").await;
    assert_eq!(second["params"], json!({"cursor": "p2"}));
}

#[tokio::test]
async fn call_tool_returns_text_and_rpc_errors() {
    let (client, _seen) = connected(Box::new(standard)).await;
    let options = RequestOptions::default();
    let result = client
        .call_tool("echo", json!({"text": "hi"}), options)
        .await
        .unwrap();
    assert_eq!(result.text, "hi");
    assert!(!result.is_error);
    let error = client
        .call_tool("fail", Value::Null, RequestOptions::default())
        .await
        .unwrap_err();
    assert!(
        matches!(error, McpError::Rpc { code: -32602, ref message, .. } if message == "bad arguments")
    );
}

#[tokio::test]
async fn null_arguments_are_sent_as_an_empty_object() {
    let (client, mut seen) = connected(Box::new(standard)).await;
    let error = client
        .call_tool("fail", Value::Null, RequestOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::Rpc { .. }));
    let call = next_with_method(&mut seen, "tools/call").await;
    assert_eq!(call["params"], json!({"name": "fail", "arguments": {}}));
}

#[tokio::test]
async fn a_response_with_a_string_id_is_matched() {
    let handler: Handler = Box::new(|message: &Value| {
        if message["method"] == "ping" {
            let id = message["id"].as_u64().unwrap().to_string();
            return vec![json!({"jsonrpc": "2.0", "id": id, "result": {}})];
        }
        standard(message)
    });
    let (client, _seen) = connected(handler).await;
    let options = RequestOptions::with_timeout(Duration::from_secs(5));
    assert!(client.ping(options).await.is_ok());
}

#[tokio::test]
async fn cancelling_a_call_notifies_the_server() {
    let (client, mut seen) = connected(Box::new(standard)).await;
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        trigger.cancel();
    });
    let options = RequestOptions::default().cancel_on(cancel);
    let error = client
        .call_tool("slow", json!({}), options)
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::Cancelled));
    let call = next_with_method(&mut seen, "tools/call").await;
    let notice = next_with_method(&mut seen, "notifications/cancelled").await;
    assert_eq!(notice["params"]["requestId"], call["id"]);
    assert_eq!(notice["params"]["reason"], "cancelled");
}

#[tokio::test]
async fn an_unanswered_request_times_out() {
    let (client, mut seen) = connected(Box::new(standard)).await;
    let error = client
        .call_tool(
            "slow",
            json!({}),
            RequestOptions::with_timeout(Duration::from_millis(50)),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::Timeout(_)));
    let notice = next_with_method(&mut seen, "notifications/cancelled").await;
    assert_eq!(notice["params"]["reason"], "timed out");
}

fn pinging_server(message: &Value) -> Vec<Value> {
    if message["method"] == "notifications/initialized" {
        return vec![
            json!({"jsonrpc": "2.0", "id": "srv-1", "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": "srv-2", "method": "sampling/createMessage", "params": {}}),
        ];
    }
    if message["method"] == "ping" {
        return vec![
            json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}),
            reply(message, &json!({})),
        ];
    }
    standard(message)
}

#[tokio::test]
async fn server_notifications_and_pings_are_handled() {
    let (client, mut seen) = connected(Box::new(pinging_server)).await;
    let mut notes = client.notifications();
    client.ping(RequestOptions::default()).await.unwrap();
    let note = tokio::time::timeout(Duration::from_secs(5), notes.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(note.method, "notifications/tools/list_changed");

    let answers = tokio::time::timeout(Duration::from_secs(5), async {
        let mut answers = Vec::new();
        while answers.len() < 2 {
            let message = seen.recv().await.unwrap();
            if message["id"] == "srv-1" || message["id"] == "srv-2" {
                answers.push(message);
            }
        }
        answers
    })
    .await
    .unwrap();
    let pong = answers.iter().find(|m| m["id"] == "srv-1").unwrap();
    assert_eq!(pong["result"], json!({}));
    let declined = answers.iter().find(|m| m["id"] == "srv-2").unwrap();
    assert_eq!(declined["error"]["code"], -32601);
}

#[tokio::test]
async fn a_closed_stream_fails_waiting_requests() {
    let (client_io, server_io) = tokio::io::duplex(1024);
    let (read, write) = tokio::io::split(client_io);
    let client = McpClient::from_stream(read, write);
    let pending = {
        let client = client.clone();
        tokio::spawn(async move { client.ping(RequestOptions::default()).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(server_io);
    let outcome = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, Err(McpError::Closed)));
    tokio::time::timeout(Duration::from_secs(5), client.closed())
        .await
        .unwrap();
    assert!(client.is_closed());
    assert!(matches!(
        client.ping(RequestOptions::default()).await,
        Err(McpError::Closed)
    ));
}

#[tokio::test]
async fn shutdown_fails_waiting_requests_and_closes() {
    let (client, _seen) = connected(Box::new(standard)).await;
    let pending = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .call_tool("slow", json!({}), RequestOptions::default())
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    tokio::time::timeout(Duration::from_secs(5), client.shutdown())
        .await
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(5), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, Err(McpError::Closed)));
    assert!(client.is_closed());
}

#[tokio::test]
async fn method_not_found_on_a_list_means_empty() {
    let handler: Handler = Box::new(|message: &Value| {
        if message["method"] == "prompts/list" {
            return vec![rpc_error(message, -32601, "no prompts")];
        }
        standard(message)
    });
    let (client, _seen) = connected(handler).await;
    assert_eq!(
        client
            .list_prompts(RequestOptions::default())
            .await
            .unwrap(),
        Vec::<systemprompt_engine::mcp::Prompt>::new()
    );
}
