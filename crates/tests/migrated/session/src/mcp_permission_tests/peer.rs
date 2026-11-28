//! A remote MCP server on a loopback port that answers over plain JSON: it
//! offers one `echo` tool and records every tool call it receives.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::task::JoinHandle;

pub(super) struct Peer {
    url: String,
    // JSON: the arguments of each tools/call the peer received.
    calls: Arc<Mutex<Vec<Value>>>,
    accept: JoinHandle<()>,
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

impl Peer {
    pub(super) async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&calls);
        let accept = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (read, write) = stream.into_split();
                tokio::spawn(serve(read, write, Arc::clone(&seen)));
            }
        });
        Self { url, calls, accept }
    }

    pub(super) fn url(&self) -> &str {
        &self.url
    }

    // JSON: the arguments of each tools/call the peer received, oldest first.
    pub(super) fn calls(&self) -> Vec<Value> {
        self.calls.lock().unwrap().clone()
    }
}

struct Request {
    method: String,
    // JSON: the JSON-RPC message the client posted.
    body: Option<Value>,
}

async fn serve(read: OwnedReadHalf, mut write: OwnedWriteHalf, calls: Arc<Mutex<Vec<Value>>>) {
    let mut reader = BufReader::new(read);
    while let Some(request) = read_request(&mut reader).await {
        let (status, body) = answer(&request, &calls);
        let mut out = format!("HTTP/1.1 {status} Peer\r\n");
        if let Some(body) = &body {
            let bytes = serde_json::to_vec(body).unwrap();
            out.push_str("content-type: application/json\r\nmcp-session-id: peer-session\r\n");
            out.push_str(&format!("content-length: {}\r\n\r\n", bytes.len()));
            let mut out = out.into_bytes();
            out.extend_from_slice(&bytes);
            if write.write_all(&out).await.is_err() {
                return;
            }
        } else {
            out.push_str("content-length: 0\r\n\r\n");
            if write.write_all(out.as_bytes()).await.is_err() {
                return;
            }
        }
    }
}

// JSON: the JSON-RPC reply to one posted message, or none for a notification.
fn answer(request: &Request, calls: &Mutex<Vec<Value>>) -> (u16, Option<Value>) {
    match request.method.as_str() {
        "GET" => return (405, None),
        "DELETE" => return (200, None),
        _ => {},
    }
    let Some(body) = &request.body else {
        return (400, None);
    };
    let result = match body["method"].as_str().unwrap_or_default() {
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "peer", "version": "1"}
        }),
        "tools/list" => json!({"tools": [{
            "name": "echo",
            "description": "Echoes its text.",
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}
        }]}),
        "tools/call" => {
            calls
                .lock()
                .unwrap()
                .push(body["params"]["arguments"].clone());
            json!({"content": [{"type": "text", "text": "echoed"}]})
        },
        "ping" => json!({}),
        _ => return (202, None),
    };
    (
        200,
        Some(json!({"jsonrpc": "2.0", "id": body["id"].clone(), "result": result})),
    )
}

async fn read_request(reader: &mut BufReader<OwnedReadHalf>) -> Option<Request> {
    let mut line = String::new();
    if reader.read_line(&mut line).await.ok()? == 0 {
        return None;
    }
    let method = line.split_whitespace().next()?.to_owned();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let length = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).await.ok()?;
    Some(Request {
        method,
        body: serde_json::from_slice(&body).ok(),
    })
}
