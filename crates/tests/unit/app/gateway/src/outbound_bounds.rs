use std::time::{Duration, Instant};

use futures_util::StreamExt;
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalReasoning, CanonicalRequest, CanonicalSampling,
    CanonicalTooling, Role,
};
use systemprompt_gateway::protocol::outbound::anthropic::AnthropicOutbound;
use systemprompt_gateway::protocol::outbound::openai_chat::OpenAiChatOutbound;
use systemprompt_gateway::protocol::outbound::{
    OutboundAdapter, OutboundCtx, OutboundError, OutboundOutcome, UpstreamBodyError,
    UpstreamBounds, UpstreamError, UpstreamHttp,
};
use systemprompt_identifiers::ModelId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::support;

const BUFFERED_BODY: &str = r#"{"id":"msg_1","type":"message","role":"assistant","model":"m","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#;

fn bounds() -> UpstreamBounds {
    UpstreamBounds {
        first_byte: Duration::from_millis(300),
        idle: Duration::from_millis(300),
        total: Duration::from_secs(5),
        max_buffered_bytes: 4096,
        max_error_body_bytes: 1024,
    }
}

fn request(stream: bool) -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("m"),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        cache_control: None,
        system: vec![],
        max_tokens: 16,
        stream,
        metadata: None,
        response_format: None,
        forwarded_surface: Default::default(),
        sampling: CanonicalSampling {
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: vec![],
            presence_penalty: None,
            frequency_penalty: None,
        },
        tooling: CanonicalTooling {
            tools: vec![],
            tool_choice: None,
            search: None,
            code_execution: false,
        },
        reasoning: CanonicalReasoning {
            thinking: None,
            reasoning_effort: None,
        },
    }
}

async fn send<A: OutboundAdapter>(
    adapter: &A,
    endpoint: &str,
    stream: bool,
    bounds: UpstreamBounds,
) -> Result<OutboundOutcome, OutboundError> {
    let hop = systemprompt_catalog_fixtures::hop(
        "anthropic",
        systemprompt_wire::WireProtocol::Anthropic,
        "https://upstream.invalid/v1",
        "m",
        "upstream-1",
    );
    let req = request(stream);
    let http = UpstreamHttp::new(reqwest::Client::new(), bounds);
    let ctx = OutboundCtx {
        target: &hop,
        upstream: &support::api_key_call(endpoint, "k"),
        http: &http,
        request: &req,
        upstream_model: "upstream-1",
        model_limits: None,
        automatic_prompt_caching: false,
        forward_headers: &[],
        raw_body: None,
    };
    let built = adapter.build_body(&ctx).expect("body builds");
    adapter.send(ctx, &built).await
}

async fn stalling_server(head: &'static str, chunks: Vec<(&'static str, Duration)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0_u8; 16 * 1024];
        let _ = socket.read(&mut buf).await;
        let _ = socket.write_all(head.as_bytes()).await;
        for (chunk, pause) in chunks {
            tokio::time::sleep(pause).await;
            if socket.write_all(chunk.as_bytes()).await.is_err() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    format!("http://{addr}/v1")
}

fn chunk(data: &str) -> String {
    format!("{:x}\r\n{data}\r\n", data.len())
}

#[tokio::test]
async fn an_upstream_that_never_answers_is_cut_at_the_first_byte_deadline() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(BUFFERED_BODY)
                .set_delay(Duration::from_secs(3)),
        )
        .mount(&server)
        .await;
    let started = Instant::now();
    let err = send(
        &AnthropicOutbound,
        &format!("{}/v1", server.uri()),
        false,
        bounds(),
    )
    .await
    .err()
    .expect("a stalled upstream must not produce a response");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    match err.upstream() {
        Some(UpstreamError::Timeout { provider, after }) => {
            assert!(provider.starts_with("anthropic"), "{provider}");
            assert_eq!(*after, Duration::from_millis(300));
        },
        other => panic!("expected an upstream timeout, got {other:?}: {err}"),
    }
}

#[tokio::test]
async fn a_buffered_body_over_the_cap_is_refused_as_too_large() {
    let server = MockServer::start().await;
    let oversized = format!(r#"{{"padding":"{}"}}"#, "x".repeat(8192));
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(oversized))
        .mount(&server)
        .await;
    let err = send(
        &AnthropicOutbound,
        &format!("{}/v1", server.uri()),
        false,
        bounds(),
    )
    .await
    .err()
    .expect("an oversized body must not be materialised");
    assert!(
        matches!(
            err,
            OutboundError::ReadBody {
                wire: "anthropic",
                source: UpstreamBodyError::TooLarge { limit: 4096 },
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_chunked_buffered_body_without_a_length_is_capped_while_it_is_read() {
    let head =
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ntransfer-encoding: chunked\r\n\r\n";
    let piece: &'static str = Box::leak(chunk(&"y".repeat(1500)).into_boxed_str());
    let endpoint = stalling_server(
        head,
        vec![
            (piece, Duration::ZERO),
            (piece, Duration::ZERO),
            (piece, Duration::ZERO),
            (piece, Duration::ZERO),
        ],
    )
    .await;
    let err = send(&OpenAiChatOutbound, &endpoint, false, bounds())
        .await
        .err()
        .expect("an oversized chunked body must not be materialised");
    assert!(
        matches!(
            err,
            OutboundError::ReadBody {
                source: UpstreamBodyError::TooLarge { limit: 4096 },
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_buffered_body_that_stalls_mid_read_ends_at_the_idle_deadline() {
    let head =
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 1000\r\n\r\n{\"id\":";
    let endpoint = stalling_server(head, vec![]).await;
    let started = Instant::now();
    let err = send(&AnthropicOutbound, &endpoint, false, bounds())
        .await
        .err()
        .expect("a stalled body must not produce a response");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert!(
        matches!(
            err,
            OutboundError::ReadBody {
                source: UpstreamBodyError::Idle { .. },
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_stream_that_stalls_between_frames_ends_with_an_error_at_the_idle_deadline() {
    let head =
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n";
    let start: &'static str = Box::leak(
        chunk(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"m\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
        )
        .into_boxed_str(),
    );
    let endpoint = stalling_server(head, vec![(start, Duration::ZERO)]).await;
    let outcome = send(&AnthropicOutbound, &endpoint, true, bounds())
        .await
        .expect("headers arrive in time");
    let OutboundOutcome::Streaming(mut events) = outcome else {
        panic!("expected a canonical stream");
    };
    let started = Instant::now();
    let mut last_error = None;
    while let Some(item) = tokio::time::timeout(Duration::from_secs(5), events.next())
        .await
        .expect("the stream must end on its own")
    {
        if let Err(e) = item {
            last_error = Some(e.to_string());
        }
    }
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    let error = last_error.expect("a stalled stream ends with an error, not a clean EOF");
    assert!(error.contains("sent nothing"), "{error}");
}

#[tokio::test]
async fn a_stream_that_trickles_past_the_total_deadline_is_cut() {
    let head =
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n";
    let ping: &'static str =
        Box::leak(chunk("event: ping\ndata: {\"type\":\"ping\"}\n\n").into_boxed_str());
    let endpoint = stalling_server(head, vec![(ping, Duration::from_millis(100)); 40]).await;
    let mut tight = bounds();
    tight.total = Duration::from_millis(800);
    let outcome = send(&AnthropicOutbound, &endpoint, true, tight)
        .await
        .expect("headers arrive in time");
    let OutboundOutcome::Streaming(mut events) = outcome else {
        panic!("expected a canonical stream");
    };
    let started = Instant::now();
    let mut errored = false;
    while let Some(item) = tokio::time::timeout(Duration::from_secs(10), events.next())
        .await
        .expect("the stream must end on its own")
    {
        errored |= item.is_err();
    }
    assert!(
        errored,
        "a stream cut by the total deadline ends with an error"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}
