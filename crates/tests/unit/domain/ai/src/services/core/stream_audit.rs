// `StreamStorageWrapper` — the audit layer wrapped around a provider stream.
//
// It accumulates text, swallows `Usage` chunks into its own totals, and spawns
// exactly one audit write when the stream finishes or errors. The completion
// and error arms both write to `ai_requests`, so the DB is the observable
// surface; the spawn is a detached task, so each assertion polls for it.

use std::time::Duration;

use futures::StreamExt;
use systemprompt_ai::models::ai::{AiMessage, AiRequest};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::ai::StreamChunk;

use super::{pool_or_skip, seeded_context, service};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const MODEL: &str = "claude-sonnet-5";

// A well-formed stream carrying both a text delta and a usage report.
const COMPLETE_SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":7,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"streamed body\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":19}}\n\n";

const TRUNCATED_SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n";

struct OwnedSseServer(Option<tokio::task::JoinHandle<()>>);

impl OwnedSseServer {
    async fn wait(&mut self) {
        let task = self.0.as_mut().expect("owned provider task");
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("owned provider finishes within timeout")
            .expect("owned provider task succeeds");
        self.0.take();
    }
}

impl Drop for OwnedSseServer {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}

async fn truncated_sse_server() -> (String, OwnedSseServer) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind owned truncated-SSE server");
    let address = listener.local_addr().expect("owned server address");
    let task = tokio::spawn(async move {
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
            .await
            .expect("provider request arrives")
            .expect("accept provider request");
        let mut request = Vec::new();
        let header_end = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let mut chunk = [0_u8; 4096];
                let read = socket
                    .read(&mut chunk)
                    .await
                    .expect("read provider request");
                assert!(read > 0, "provider closed before sending request headers");
                request.extend_from_slice(&chunk[..read]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    assert!(
                        end + 4 <= 64 * 1024,
                        "provider request headers exceed 64 KiB"
                    );
                    break end + 4;
                }
                assert!(
                    request.len() <= 64 * 1024,
                    "provider request headers exceed 64 KiB"
                );
            }
        })
        .await
        .expect("provider headers arrive within timeout");
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("numeric content length")
                })
            })
            .expect("provider request declares content length");
        tokio::time::timeout(Duration::from_secs(10), async {
            while request.len() < header_end + content_length {
                let mut chunk = [0_u8; 4096];
                let read = socket.read(&mut chunk).await.expect("read provider body");
                assert!(read > 0, "provider closed before sending full request body");
                request.extend_from_slice(&chunk[..read]);
            }
        })
        .await
        .expect("provider body arrives within timeout");
        assert!(
            String::from_utf8_lossy(&request).starts_with("POST "),
            "AI provider request uses POST"
        );
        let declared = TRUNCATED_SSE.len() + 128;
        let headers = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {declared}\r\nconnection: close\r\n\r\n"
        );
        tokio::time::timeout(Duration::from_secs(10), async {
            socket
                .write_all(headers.as_bytes())
                .await
                .expect("write response headers");
            socket
                .write_all(TRUNCATED_SSE.as_bytes())
                .await
                .expect("write partial SSE frames");
            socket.shutdown().await.expect("truncate response body");
        })
        .await
        .expect("provider response writes within timeout");
    });
    (format!("http://{address}"), OwnedSseServer(Some(task)))
}

fn request(context: systemprompt_models::RequestContext) -> AiRequest {
    AiRequest::builder(
        vec![AiMessage::user("stream please")],
        ANTHROPIC,
        MODEL,
        128,
        context,
    )
    .build()
}

async fn wait_for_audit(pool: &DbPool, user_id: &UserId, status: &str) -> i64 {
    let read = pool.pool_arc().expect("read pool");
    let mut count = 0_i64;
    for _ in 0..100 {
        count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1 AND status = $2",
            user_id.as_str(),
            status
        )
        .fetch_one(read.as_ref())
        .await
        .expect("count audit rows")
        .unwrap_or(0);
        if count > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    count
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_completed_stream_audits_once_with_the_accumulated_text_and_usage() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(COMPLETE_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    let mut stream = svc
        .generate_stream(&request(context))
        .await
        .expect("stream opens");

    let mut text = String::new();
    let mut usage_chunks = 0;
    while let Some(item) = stream.next().await {
        match item.expect("stream item") {
            StreamChunk::Text(t) => text.push_str(&t),
            StreamChunk::Usage { .. } => usage_chunks += 1,
        }
    }
    assert_eq!(text, "streamed body");
    assert_eq!(
        usage_chunks, 0,
        "the wrapper absorbs usage frames into its own audit rather than \
         forwarding them to the caller"
    );

    assert_eq!(
        wait_for_audit(&pool, &user, "completed").await,
        1,
        "a completed stream must write exactly one completed audit row"
    );

    let tokens: Option<i32> = sqlx::query_scalar!(
        "SELECT output_tokens FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert_eq!(
        tokens,
        Some(19),
        "the usage frame the caller never saw must still reach the audit row"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_truncated_stream_surfaces_the_error_and_persists_failed_zero_cost_usage() {
    let pool = pool_or_skip()
        .await
        .expect("AI stream audit database fixture");
    let (endpoint, mut provider) = truncated_sse_server().await;
    let svc = service(&pool, ANTHROPIC, endpoint);
    let (user, context) = seeded_context(&pool).await;

    let mut stream = svc
        .generate_stream(&request(context))
        .await
        .expect("the stream opens before the provider truncates it");

    let mut text = String::new();
    let error = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match stream.next().await {
                Some(Ok(StreamChunk::Text(delta))) => text.push_str(&delta),
                Some(Ok(StreamChunk::Usage { .. })) => {
                    panic!("usage is absorbed by the audit wrapper")
                },
                Some(Err(error)) => break error,
                None => panic!("a short content-length body must not complete successfully"),
            }
        }
    })
    .await
    .expect("provider truncation reaches the stream consumer");
    assert_eq!(text, "partial");
    let diagnosis = error.to_string();
    assert!(
        diagnosis.contains("Stream error:") && diagnosis.contains("body"),
        "transport truncation retains the response-body diagnosis: {diagnosis}"
    );
    drop(stream);
    provider.wait().await;
    svc.audit_tasks().close();
    tokio::time::timeout(Duration::from_secs(10), svc.audit_tasks().wait())
        .await
        .expect("stream audit tasks drain within timeout");

    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<i32>,
            i64,
            bool,
        ),
    >(
        "SELECT status,error_message,input_tokens,output_tokens,tokens_used,cost_microdollars,is_streaming \
         FROM ai_requests WHERE user_id=$1",
    )
    .bind(user.as_str())
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("failed stream audit row");
    assert_eq!(row.0, "failed");
    assert_eq!(row.1.as_deref(), Some(diagnosis.as_str()));
    assert_eq!(
        (row.2, row.3, row.4),
        (None, None, None),
        "usage remains unknown because truncation occurred before Anthropic emitted its canonical usage chunk"
    );
    assert_eq!(row.5, 0, "provider stream errors record zero cost");
    assert!(row.6);

    let assistant_messages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_request_messages m JOIN ai_requests r ON r.id=m.request_id \
         WHERE r.user_id=$1 AND m.role='assistant'",
    )
    .bind(user.as_str())
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("count assistant messages");
    assert_eq!(
        assistant_messages, 0,
        "partial output reaches the consumer but is not fabricated as a completed assistant turn"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_tooled_stream_wrapper_audits_on_the_same_terms() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(COMPLETE_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    let mut stream = svc
        .generate_with_tools_stream(&request(context))
        .await
        .expect("tooled stream opens");
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        if let StreamChunk::Text(t) = item.expect("stream item") {
            text.push_str(&t);
        }
    }
    assert_eq!(text, "streamed body");

    assert_eq!(
        wait_for_audit(&pool, &user, "completed").await,
        1,
        "the tooled streaming path shares the same audit wrapper"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stream_dropped_before_completion_does_not_audit_a_completion() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(COMPLETE_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    {
        let mut stream = svc
            .generate_stream(&request(context))
            .await
            .expect("stream opens");
        let first = stream.next().await.expect("at least one chunk");
        assert!(first.is_ok());
        // Dropped without draining: the wrapper only audits on a terminal poll.
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
    let completed = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1 AND status = 'completed'",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(
        completed, 0,
        "an abandoned stream never reached its terminal poll, so nothing may be \
         recorded as completed"
    );
}

// An SSE body carrying the framing noise a real upstream emits: comment
// keep-alives, blank lines, and an event line with no `data:` prefix. The
// wrapper must skip all of it and still accumulate the deltas.
const NOISY_SSE: &str = ": keepalive\n\nevent: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":2,\"output_tokens\":1}}}\n\n: another comment\n\ndata: not-json-at-all\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"through the noise\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":4}}\n\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sse_framing_noise_is_skipped_without_breaking_the_stream() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(NOISY_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    let mut stream = svc
        .generate_stream(&request(context))
        .await
        .expect("stream opens");
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        if let StreamChunk::Text(t) = item.expect("noise must not surface as an error") {
            text.push_str(&t);
        }
    }

    assert_eq!(
        text, "through the noise",
        "comment lines, blank lines, non-data lines and undecodable payloads must all \
         be skipped rather than aborting the stream or leaking into the text"
    );
    assert_eq!(
        wait_for_audit(&pool, &user, "completed").await,
        1,
        "a stream that survived the noise must still audit as completed"
    );
}
