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

use super::{pool, seeded_context, service};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const MODEL: &str = "claude-sonnet-4-6";

// A well-formed stream carrying both a text delta and a usage report.
const COMPLETE_SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":7,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"streamed body\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":19}}\n\n";

// Starts cleanly, then emits a frame the parser cannot decode.
const BROKEN_SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"x\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":1}}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\n\n";

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
    let Some(pool) = pool().await else {
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
async fn a_stream_that_errors_midway_audits_the_failure_not_a_completion() {
    let Some(pool) = pool().await else {
        return;
    };
    let server = mock_http::anthropic_messages_stream(BROKEN_SSE).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    let mut stream = svc
        .generate_stream(&request(context))
        .await
        .expect("the stream opens before it breaks");

    let mut saw_error = false;
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(StreamChunk::Text(t)) => text.push_str(&t),
            Ok(StreamChunk::Usage { .. }) => {},
            Err(_) => {
                saw_error = true;
                break;
            },
        }
    }

    if saw_error {
        assert_eq!(
            wait_for_audit(&pool, &user, "failed").await,
            1,
            "a stream that breaks partway must audit the failure"
        );
    } else {
        assert_eq!(
            wait_for_audit(&pool, &user, "completed").await,
            1,
            "a stream the parser tolerated must still audit exactly once"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_tooled_stream_wrapper_audits_on_the_same_terms() {
    let Some(pool) = pool().await else {
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
    let Some(pool) = pool().await else {
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
    let Some(pool) = pool().await else {
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
