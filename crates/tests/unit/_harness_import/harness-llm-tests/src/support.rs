use bytes::Bytes;
use futures_util::StreamExt;
use harness_llm::{
    ByteStream, Endpoint, Event, EventStream, HttpRequest, HttpResponse, LlmError, ModelSpec,
    Protocol, Request, Supports, Transport, TransportError, stream,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use url::Url;

pub(crate) const BASE: &str = "https://gw.example/v1";

pub(crate) struct Canned {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) chunks: Vec<Bytes>,
    pub(crate) hang: bool,
}

impl Canned {
    pub(crate) fn new(status: u16, chunks: &[&str]) -> Self {
        let chunks = chunks
            .iter()
            .map(|c| Bytes::copy_from_slice(c.as_bytes()))
            .collect();
        Self {
            status,
            headers: Vec::new(),
            chunks,
            hang: false,
        }
    }

    pub(crate) fn split(status: u16, body: &str, size: usize) -> Self {
        let chunks = body
            .as_bytes()
            .chunks(size)
            .map(Bytes::copy_from_slice)
            .collect();
        Self {
            status,
            headers: Vec::new(),
            chunks,
            hang: false,
        }
    }
}

#[async_trait::async_trait]
impl Transport for Canned {
    async fn send(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let chunks: Vec<Result<Bytes, TransportError>> =
            self.chunks.iter().cloned().map(Ok).collect();
        let body: ByteStream = if self.hang {
            Box::pin(futures_util::stream::iter(chunks).chain(futures_util::stream::pending()))
        } else {
            Box::pin(futures_util::stream::iter(chunks))
        };
        Ok(HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body,
        })
    }
}

pub(crate) fn model(protocol: Protocol) -> ModelSpec {
    ModelSpec {
        id: "m".into(),
        name: "M".into(),
        protocol,
        context_window: 100_000,
        max_output: 1_000,
        supports: Supports::default(),
    }
}

pub(crate) fn endpoint(base: &str) -> Endpoint {
    Endpoint {
        base_url: Url::parse(base).unwrap(),
        headers: vec![("authorization".into(), "Bearer secret".into())],
        metadata: Some(json!({"metadata": {"user_id": "{\"session_id\":\"s\"}"}})),
    }
}

pub(crate) async fn open(transport: &Canned, protocol: Protocol) -> Result<EventStream, LlmError> {
    let request = Request::new(model(protocol));
    stream(
        transport,
        &endpoint(BASE),
        &request,
        CancellationToken::new(),
    )
    .await
    .map(|response| response.events)
}

pub(crate) async fn run(transport: &Canned, protocol: Protocol) -> Vec<Result<Event, LlmError>> {
    let Ok(events) = open(transport, protocol).await else {
        panic!("the canned response did not open a stream");
    };
    events.collect().await
}

pub(crate) async fn decode(
    protocol: Protocol,
    body: &str,
    size: usize,
) -> Result<Vec<Event>, LlmError> {
    run(&Canned::split(200, body, size), protocol)
        .await
        .into_iter()
        .collect()
}
