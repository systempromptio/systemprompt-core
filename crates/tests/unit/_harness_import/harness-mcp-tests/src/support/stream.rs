use std::time::Duration;

use harness_mcp::{ClientInfo, McpClient};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::mpsc;

pub(crate) type Handler = Box<dyn Fn(&Value) -> Vec<Value> + Send>;

pub(crate) fn fake_server(io: DuplexStream, handler: Handler) -> mpsc::UnboundedReceiver<Value> {
    let (seen_tx, seen_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(io);
        let mut lines = BufReader::new(read).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let message: Value = serde_json::from_str(&line).unwrap();
            for reply in handler(&message) {
                let mut text = serde_json::to_vec(&reply).unwrap();
                text.push(b'\n');
                if write.write_all(&text).await.is_err() {
                    return;
                }
            }
            if seen_tx.send(message).is_err() {
                return;
            }
        }
    });
    seen_rx
}

pub(crate) fn reply(message: &Value, result: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": message["id"].clone(), "result": result})
}

pub(crate) fn rpc_error(message: &Value, code: i64, text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": message["id"].clone(),
        "error": {"code": code, "message": text},
    })
}

pub(crate) fn initialize_result(version: &str) -> Value {
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": true}},
        "serverInfo": {"name": "fake", "version": "0.0.1"},
        "instructions": "  use the fake tools  ",
    })
}

pub(crate) fn standard(message: &Value) -> Vec<Value> {
    match message["method"].as_str().unwrap_or_default() {
        "initialize" => vec![reply(message, &initialize_result("2025-06-18"))],
        "tools/list" => match message["params"]["cursor"].as_str() {
            None => vec![reply(
                message,
                &json!({"tools": [{"name": "echo", "inputSchema": {"type": "object"}}], "nextCursor": "p2"}),
            )],
            Some("p2") => vec![reply(
                message,
                &json!({"tools": [{"name": "add", "description": "adds"}]}),
            )],
            Some(_) => vec![reply(message, &json!({"tools": []}))],
        },
        "tools/call" if message["params"]["name"] == "echo" => vec![reply(
            message,
            &json!({"content": [{"type": "text", "text": message["params"]["arguments"]["text"].clone()}]}),
        )],
        "tools/call" if message["params"]["name"] == "fail" => {
            vec![rpc_error(message, -32602, "bad arguments")]
        },
        "ping" => vec![reply(message, &json!({}))],
        _ => Vec::new(),
    }
}

pub(crate) async fn connected(handler: Handler) -> (McpClient, mpsc::UnboundedReceiver<Value>) {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let seen = fake_server(server_io, handler);
    let (read, write) = tokio::io::split(client_io);
    let client = McpClient::from_stream(read, write);
    client
        .initialize(&ClientInfo::new("tester", "1.0"), None)
        .await
        .unwrap();
    (client, seen)
}

pub(crate) async fn next_with_method(
    seen: &mut mpsc::UnboundedReceiver<Value>,
    method: &str,
) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let message = seen.recv().await.unwrap();
            if message["method"] == method {
                return message;
            }
        }
    })
    .await
    .unwrap()
}
