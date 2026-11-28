//! Shared fixtures: a canned transport, model specs, stream builders for both
//! protocols and a one-request loopback HTTP server.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{Value, json};
use systemprompt_engine::llm::{
    ByteStream, Endpoint, Event, HttpRequest, HttpResponse, LlmError, ModelSpec, Protocol, Request,
    Supports, Transport, TransportError, request_body, stream,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use url::Url;

pub(crate) const BASE: &str = "https://gateway.test/v1";

/// One canned answer: a status, headers and body chunks, optionally followed by
/// a failure or by silence.
pub(crate) struct Canned {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) chunks: Vec<Bytes>,
    pub(crate) then_fail: bool,
    pub(crate) then_hang: bool,
    pub(crate) sends: Arc<AtomicU32>,
}

impl Canned {
    pub(crate) fn body(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            chunks: vec![Bytes::copy_from_slice(body.as_bytes())],
            then_fail: false,
            then_hang: false,
            sends: Arc::new(AtomicU32::new(0)),
        }
    }

    pub(crate) fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub(crate) fn failing(mut self) -> Self {
        self.then_fail = true;
        self
    }

    pub(crate) fn hanging(mut self) -> Self {
        self.then_hang = true;
        self
    }

    pub(crate) fn sent(&self) -> u32 {
        self.sends.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl Transport for Canned {
    async fn send(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        let mut items: Vec<Result<Bytes, TransportError>> =
            self.chunks.iter().cloned().map(Ok).collect();
        if self.then_fail {
            items.push(Err(TransportError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "connection reset by peer",
            ))));
        }
        let body: ByteStream = if self.then_hang {
            Box::pin(futures_util::stream::iter(items).chain(futures_util::stream::pending()))
        } else {
            Box::pin(futures_util::stream::iter(items))
        };
        Ok(HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body,
        })
    }
}

pub(crate) fn model(protocol: Protocol, supports: Supports) -> ModelSpec {
    ModelSpec {
        id: "model-under-test".into(),
        name: "Model under test".into(),
        protocol,
        context_window: 200_000,
        max_output: 8_000,
        supports,
    }
}

pub(crate) const fn all_supports() -> Supports {
    Supports {
        images: true,
        reasoning: true,
        tool_calls: true,
        caching: true,
    }
}

pub(crate) fn request(protocol: Protocol, supports: Supports) -> Request {
    Request::new(model(protocol, supports))
}

pub(crate) fn body(request: &Request) -> Value {
    request_body(request, None)
}

pub(crate) fn endpoint() -> Endpoint {
    Endpoint::new(Url::parse(BASE).unwrap())
}

/// Sends one request through `transport` and collects every event or error.
pub(crate) async fn collect(
    transport: &dyn Transport,
    protocol: Protocol,
) -> Result<Vec<Result<Event, LlmError>>, LlmError> {
    let request = request(protocol, Supports::default());
    let response = stream(transport, &endpoint(), &request, CancellationToken::new()).await?;
    Ok(response.events.collect().await)
}

pub(crate) async fn decode(protocol: Protocol, body: &str) -> Vec<Result<Event, LlmError>> {
    collect(&Canned::body(200, body), protocol).await.unwrap()
}

pub(crate) async fn decode_ok(protocol: Protocol, body: &str) -> Vec<Event> {
    decode(protocol, body)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

pub(crate) async fn status_error(status: u16, body: &str) -> LlmError {
    status_error_with(Canned::body(status, body)).await
}

pub(crate) async fn status_error_with(canned: Canned) -> LlmError {
    match collect(&canned, Protocol::Messages).await {
        Err(error) => error,
        Ok(events) => panic!("a refused call opened a stream: {events:?}"),
    }
}

/// A messages-protocol stream: one SSE frame per event, named by its type.
pub(crate) fn messages_sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|event| {
            let kind = event["type"].as_str().unwrap_or_default();
            format!("event: {kind}\ndata: {event}\n\n")
        })
        .collect()
}

pub(crate) fn message_start(input: u64) -> Value {
    json!({"type": "message_start", "message": {"id": "msg_1", "usage": {"input_tokens": input, "output_tokens": 1}}})
}

pub(crate) fn message_end(reason: &str, output: u64) -> [Value; 2] {
    [
        json!({"type": "message_delta", "delta": {"stop_reason": reason}, "usage": {"output_tokens": output}}),
        json!({"type": "message_stop"}),
    ]
}

pub(crate) fn text_block(index: u64, text: &str) -> [Value; 3] {
    [
        json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": text}}),
        json!({"type": "content_block_stop", "index": index}),
    ]
}

pub(crate) fn tool_block(index: u64, id: &str, name: &str, fragments: &[&str]) -> Vec<Value> {
    let mut out = vec![json!({
        "type": "content_block_start", "index": index,
        "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}
    })];
    for fragment in fragments {
        out.push(json!({
            "type": "content_block_delta", "index": index,
            "delta": {"type": "input_json_delta", "partial_json": fragment}
        }));
    }
    out.push(json!({"type": "content_block_stop", "index": index}));
    out
}

/// A complete messages-protocol answer from its content events.
pub(crate) fn messages_answer(content: &[Value], reason: &str) -> String {
    let mut events = vec![message_start(10)];
    events.extend_from_slice(content);
    events.extend(message_end(reason, 5));
    messages_sse(&events)
}

/// A chat-completions stream: one `data:` frame per chunk, then `[DONE]`
/// when `done`.
pub(crate) fn chat_sse(chunks: &[Value], done: bool) -> String {
    let mut out: String = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect();
    if done {
        out.push_str("data: [DONE]\n\n");
    }
    out
}

pub(crate) fn chat_delta(delta: &Value) -> Value {
    json!({"choices": [{"index": 0, "delta": delta, "finish_reason": null}]})
}

pub(crate) fn chat_finish(reason: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]})
}

pub(crate) fn texts(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::TextDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub(crate) fn finish(events: &[Event]) -> Option<&systemprompt_engine::llm::FinishReason> {
    events.iter().find_map(|event| match event {
        Event::Finish { reason } => Some(reason),
        _ => None,
    })
}

pub(crate) fn tool_ends(events: &[Event]) -> Vec<(String, String, Value, Option<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ToolCallEnd {
                id,
                name,
                input,
                malformed,
            } => Some((id.clone(), name.clone(), input.clone(), malformed.clone())),
            _ => None,
        })
        .collect()
}

pub(crate) fn errors(items: &[Result<Event, LlmError>]) -> Vec<&LlmError> {
    items
        .iter()
        .filter_map(|item| item.as_ref().err())
        .collect()
}

pub(crate) fn oks(items: &[Result<Event, LlmError>]) -> Vec<Event> {
    items
        .iter()
        .filter_map(|item| item.as_ref().ok().cloned())
        .collect()
}

/// A loopback HTTP server answering each accepted connection with the next
/// canned response, in order; it hands back each request head it read.
pub(crate) struct Loopback {
    pub(crate) url: String,
    pub(crate) heads: tokio::sync::mpsc::UnboundedReceiver<String>,
}

pub(crate) async fn loopback(responses: Vec<String>) -> Loopback {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, heads) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        for response in responses {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let head = read_head(&mut socket).await;
            if tx.send(head).is_err() {
                return;
            }
            if response.is_empty() {
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    drop(socket);
                });
                continue;
            }
            if socket.write_all(response.as_bytes()).await.is_err() {
                return;
            }
            drop(socket.shutdown().await);
        }
    });
    Loopback { url, heads }
}

async fn read_head(socket: &mut tokio::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Ok(read) = socket.read(&mut chunk).await {
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..end]).into_owned();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            while buffer.len() < end + 4 + length {
                match socket.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                }
            }
            return head;
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

/// A loopback server whose every answer redirects back to itself, `hops`
/// times; hands back its URL.
pub(crate) async fn redirect_loop(hops: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/again", listener.local_addr().unwrap());
    let location = url.clone();
    tokio::spawn(async move {
        for _ in 0..hops {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            read_head(&mut socket).await;
            let response = http_response(302, &[("location", location.as_str())], "");
            if socket.write_all(response.as_bytes()).await.is_err() {
                return;
            }
            drop(socket.shutdown().await);
        }
    });
    url
}

pub(crate) fn http_response(status: u16, headers: &[(&str, &str)], body: &str) -> String {
    let mut out = format!(
        "HTTP/1.1 {status} Status\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    out.push_str(body);
    out
}

pub(crate) fn post(url: &str) -> HttpRequest {
    HttpRequest {
        url: Url::parse(url).unwrap(),
        headers: vec![("authorization".to_owned(), "Bearer secret-token".to_owned())],
        body: Bytes::from_static(b"{}"),
    }
}
