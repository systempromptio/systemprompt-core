use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::stream::initialize_result;

#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Option<Value>,
}

impl Recorded {
    pub(crate) fn rpc_method(&self) -> Option<&str> {
        self.body.as_ref().and_then(|body| body["method"].as_str())
    }

    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

pub(crate) enum Body {
    Bytes(Vec<u8>),
    Stream(mpsc::UnboundedReceiver<Vec<u8>>),
}

pub(crate) struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Body,
}

impl Response {
    pub(crate) fn json(value: &Value) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".into(), "application/json".into())],
            body: Body::Bytes(serde_json::to_vec(value).unwrap()),
        }
    }

    pub(crate) fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Body::Bytes(Vec::new()),
        }
    }

    pub(crate) fn stream(chunks: mpsc::UnboundedReceiver<Vec<u8>>) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".into(), "text/event-stream".into())],
            body: Body::Stream(chunks),
        }
    }

    pub(crate) fn event_stream(chunks: &[&str]) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        for chunk in chunks {
            tx.send(chunk.as_bytes().to_vec()).unwrap();
        }
        Self::stream(rx)
    }

    pub(crate) fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

type Handler = Arc<dyn Fn(&Recorded) -> Response + Send + Sync>;

pub(crate) struct HttpServer {
    base: String,
    log: Arc<Mutex<Vec<Recorded>>>,
    accept: JoinHandle<()>,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

impl HttpServer {
    pub(crate) async fn start<H>(handler: H) -> Self
    where
        H: Fn(&Recorded) -> Response + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let handler: Handler = Arc::new(handler);
        let accept_log = Arc::clone(&log);
        let accept = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (read, write) = stream.into_split();
                tokio::spawn(serve(
                    read,
                    write,
                    Arc::clone(&handler),
                    Arc::clone(&accept_log),
                ));
            }
        });
        Self { base, log, accept }
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub(crate) fn requests(&self) -> Vec<Recorded> {
        self.log.lock().unwrap().clone()
    }

    pub(crate) fn rpc_count(&self, path: &str, method: &str) -> usize {
        self.requests()
            .iter()
            .filter(|request| request.path == path && request.rpc_method() == Some(method))
            .count()
    }

    pub(crate) fn last_rpc(&self, path: &str, method: &str) -> Option<Recorded> {
        self.requests()
            .into_iter()
            .rev()
            .find(|request| request.path == path && request.rpc_method() == Some(method))
    }
}

async fn serve(
    read: OwnedReadHalf,
    mut write: OwnedWriteHalf,
    handler: Handler,
    log: Arc<Mutex<Vec<Recorded>>>,
) {
    let mut reader = BufReader::new(read);
    while let Some(request) = read_request(&mut reader).await {
        log.lock().unwrap().push(request.clone());
        let response = handler(&request);
        if !write_response(&mut write, response).await {
            return;
        }
    }
}

async fn read_request(reader: &mut BufReader<OwnedReadHalf>) -> Option<Recorded> {
    let mut line = String::new();
    if reader.read_line(&mut line).await.ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
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
    let body = serde_json::from_slice(&body).ok();
    Some(Recorded {
        method,
        path,
        headers,
        body,
    })
}

async fn write_response(write: &mut OwnedWriteHalf, response: Response) -> bool {
    let mut head = format!("HTTP/1.1 {} Fake\r\n", response.status);
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    match response.body {
        Body::Bytes(bytes) => {
            head.push_str(&format!("content-length: {}\r\n\r\n", bytes.len()));
            let mut out = head.into_bytes();
            out.extend_from_slice(&bytes);
            write.write_all(&out).await.is_ok()
        },
        Body::Stream(mut chunks) => {
            head.push_str("transfer-encoding: chunked\r\n\r\n");
            if write.write_all(head.as_bytes()).await.is_err() {
                return false;
            }
            while let Some(chunk) = chunks.recv().await {
                let framed = [
                    format!("{:x}\r\n", chunk.len()).into_bytes(),
                    chunk,
                    b"\r\n".to_vec(),
                ];
                if write.write_all(&framed.concat()).await.is_err() || write.flush().await.is_err()
                {
                    return false;
                }
            }
            write.write_all(b"0\r\n\r\n").await.is_ok()
        },
    }
}

pub(crate) fn rpc_frame(request: &Recorded, result: &Value) -> Value {
    let id = request
        .body
        .as_ref()
        .map_or(Value::Null, |body| body["id"].clone());
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_result(request: &Recorded, result: &Value) -> Response {
    Response::json(&rpc_frame(request, result))
}

pub(crate) fn mcp_reply(request: &Recorded, tools: &Value) -> Response {
    match request.method.as_str() {
        "GET" => return Response::status(405),
        "DELETE" => return Response::status(200),
        _ => {},
    }
    let Some(body) = &request.body else {
        return Response::status(400);
    };
    match request.rpc_method().unwrap_or_default() {
        "initialize" => rpc_result(request, &initialize_result("2025-06-18"))
            .with_header("mcp-session-id", "session-1"),
        "tools/list" => rpc_result(request, &json!({"tools": tools})),
        "tools/call" => {
            let text = format!(
                "called {}",
                body["params"]["name"].as_str().unwrap_or_default()
            );
            rpc_result(
                request,
                &json!({"content": [{"type": "text", "text": text}]}),
            )
        },
        "ping" => rpc_result(request, &json!({})),
        _ => Response::status(202),
    }
}

pub(crate) fn tools(names: &[&str]) -> Value {
    Value::Array(names.iter().map(|name| json!({"name": name})).collect())
}
