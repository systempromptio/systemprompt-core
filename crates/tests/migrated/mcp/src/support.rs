//! An in-process peer that speaks newline-delimited JSON-RPC over a duplex
//! pipe, answering each client message with a handler.

use serde_json::{Value, json};
use systemprompt_engine::mcp::{ClientInfo, McpClient};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::mpsc;

pub(crate) type Handler = Box<dyn Fn(&Value) -> Vec<Value> + Send>;

pub(crate) fn serve(io: DuplexStream, handler: Handler) -> mpsc::UnboundedReceiver<Value> {
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

pub(crate) fn method(message: &Value) -> &str {
    message["method"].as_str().unwrap_or_default()
}

pub(crate) fn initialize(message: &Value, capabilities: &Value) -> Vec<Value> {
    vec![reply(
        message,
        &json!({"protocolVersion": "2025-06-18", "capabilities": capabilities, "serverInfo": {"name": "peer", "version": "1"}}),
    )]
}

/// A client connected to a peer and initialized; also hands back every
/// message the peer received.
pub(crate) async fn connected(handler: Handler) -> (McpClient, mpsc::UnboundedReceiver<Value>) {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let seen = serve(server_io, handler);
    let (read, write) = tokio::io::split(client_io);
    let client = McpClient::from_stream(read, write);
    client
        .initialize(&ClientInfo::new("migrated-mcp-tests", "0"), None)
        .await
        .unwrap();
    (client, seen)
}

pub(crate) fn drain(seen: &mut mpsc::UnboundedReceiver<Value>) -> Vec<String> {
    let mut methods = Vec::new();
    while let Ok(message) = seen.try_recv() {
        methods.push(method(&message).to_owned());
    }
    methods
}
