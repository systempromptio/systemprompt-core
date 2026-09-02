//! Unit tests for the thought-signature cache: re-injection of Gemini
//! `thoughtSignature` values into tool_use blocks stripped by strict
//! Anthropic-protocol clients, persisted so a second cache instance (another
//! replica) sees what the first captured.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_ai::repository::AiThoughtSignatureRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::GatewayConversationId;

use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};
use systemprompt_api::services::gateway::signature_cache::ThoughtSignatureCache;
use systemprompt_models::profile::WireProtocol;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

const TTL: Duration = Duration::from_secs(60);

struct Harness {
    pool: DbPool,
    repository: Arc<AiThoughtSignatureRepository>,
}

impl Harness {
    async fn open() -> Option<Self> {
        let url = fixture_database_url().ok()?;
        let pool = fixture_db_pool(&url).await.ok()?;
        let repository = Arc::new(AiThoughtSignatureRepository::new(&pool).expect("repository"));
        Some(Self { pool, repository })
    }

    fn cache(&self) -> ThoughtSignatureCache {
        self.cache_with_ttl(TTL)
    }

    fn cache_with_ttl(&self, ttl: Duration) -> ThoughtSignatureCache {
        ThoughtSignatureCache::new(ttl, Arc::clone(&self.repository))
    }

    async fn expire_in_db(&self, conversation: &GatewayConversationId, tool_use_id: &str) {
        let write = self.pool.write_pool_arc().unwrap();
        sqlx::query(
            "UPDATE ai_gateway_thought_signatures SET expires_at = NOW() - INTERVAL '1 hour' \
             WHERE conversation_id = $1 AND tool_use_id = $2",
        )
        .bind(conversation.as_str())
        .bind(tool_use_id)
        .execute(write.as_ref())
        .await
        .unwrap();
    }
}

const GEMINI: Option<WireProtocol> = Some(WireProtocol::Gemini);

fn conv() -> GatewayConversationId {
    GatewayConversationId::new_unchecked(format!(
        "ctx_{:016x}",
        u64::from(uuid::Uuid::new_v4().as_u128() as u32)
    ))
}

fn tool_use(id: &str, signature: Option<&str>) -> CanonicalContent {
    CanonicalContent::ToolUse {
        id: id.to_owned(),
        name: "lookup".to_owned(),
        input: serde_json::json!({"q": "x"}),
        signature: signature.map(str::to_owned),
    }
}

fn request_with(content: Vec<CanonicalContent>) -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::Assistant,
            content,
        }],
        max_tokens: 10,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

fn response_with(content: Vec<CanonicalContent>) -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "m".into(),
        content,
        stop_reason: Some(CanonicalStopReason::ToolUse),
        usage: CanonicalUsage::default(),
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
        ..Default::default()
    }
}

fn signature_of(request: &CanonicalRequest) -> Option<String> {
    request.messages.iter().find_map(|m| {
        m.content.iter().find_map(|c| match c {
            CanonicalContent::ToolUse { signature, .. } => signature.clone(),
            _ => None,
        })
    })
}

#[tokio::test]
async fn hydrate_injects_cached_signature_when_none() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let conv = conv();
    cache.store(&conv, "call_1", "sig-a").await;
    let mut request = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&conv, &mut request, GEMINI).await;
    assert_eq!(signature_of(&request).as_deref(), Some("sig-a"));
}

#[tokio::test]
async fn hydrate_passthrough_on_miss() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let mut request = request_with(vec![tool_use("call_unknown", None)]);
    cache.hydrate_request(&conv(), &mut request, GEMINI).await;
    assert_eq!(signature_of(&request), None);
}

#[tokio::test]
async fn inbound_signature_wins_and_rewarms() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let conv = conv();
    cache.store(&conv, "call_1", "cached").await;
    let mut request = request_with(vec![tool_use("call_1", Some("client"))]);
    cache.hydrate_request(&conv, &mut request, GEMINI).await;
    assert_eq!(signature_of(&request).as_deref(), Some("client"));
    assert_eq!(
        cache.lookup(&conv, "call_1").await.as_deref(),
        Some("client")
    );
    assert_eq!(
        h.cache().lookup(&conv, "call_1").await.as_deref(),
        Some("client"),
        "rewarm must reach the database, not just this instance"
    );
}

#[tokio::test]
async fn a_signature_stored_by_one_instance_is_found_by_another() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let conv = conv();
    let replica_a = h.cache();
    let replica_b = h.cache();
    replica_a.store(&conv, "call_1", "sig-a").await;
    assert_eq!(
        replica_b.lookup(&conv, "call_1").await.as_deref(),
        Some("sig-a")
    );

    let mut replay = request_with(vec![tool_use("call_1", None)]);
    replica_b.hydrate_request(&conv, &mut replay, GEMINI).await;
    assert_eq!(signature_of(&replay).as_deref(), Some("sig-a"));
}

#[tokio::test]
async fn db_expiry_drops_entry_for_a_fresh_instance() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let conv = conv();
    h.cache().store(&conv, "call_1", "sig-a").await;
    h.expire_in_db(&conv, "call_1").await;
    assert_eq!(h.cache().lookup(&conv, "call_1").await, None);
}

#[tokio::test]
async fn local_ttl_expiry_falls_through_to_the_database() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let conv = conv();
    let cache = h.cache_with_ttl(Duration::from_millis(1));
    cache.store(&conv, "call_1", "sig-a").await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(cache.lookup(&conv, "call_1").await, None);
}

#[tokio::test]
async fn lookup_refreshes_ttl() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let conv = conv();
    let cache = h.cache_with_ttl(Duration::from_millis(60));
    cache.store(&conv, "call_1", "sig-a").await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        cache.lookup(&conv, "call_1").await.as_deref(),
        Some("sig-a")
    );
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(
        cache.lookup(&conv, "call_1").await.as_deref(),
        Some("sig-a")
    );
}

#[tokio::test]
async fn store_from_response_caches_only_signed_tool_use() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let conv = conv();
    let response = response_with(vec![
        CanonicalContent::Text("hi".to_owned()),
        tool_use("call_signed", Some("sig-a")),
        tool_use("call_unsigned", None),
    ]);
    cache.store_from_response(&conv, &response).await;
    assert_eq!(
        cache.lookup(&conv, "call_signed").await.as_deref(),
        Some("sig-a")
    );
    assert_eq!(cache.lookup(&conv, "call_unsigned").await, None);
}

#[tokio::test]
async fn response_signatures_survive_a_stripped_replay() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let conv = conv();
    cache
        .store_from_response(
            &conv,
            &response_with(vec![tool_use("call_1", Some("sig-a"))]),
        )
        .await;
    let mut replay = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&conv, &mut replay, GEMINI).await;
    assert_eq!(signature_of(&replay).as_deref(), Some("sig-a"));
}

#[tokio::test]
async fn signatures_are_scoped_to_their_conversation() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let scoped = conv();
    cache.store(&scoped, "call_1", "sig-a").await;
    let other = conv();
    assert_eq!(cache.lookup(&other, "call_1").await, None);
    let mut request = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&other, &mut request, GEMINI).await;
    assert_eq!(signature_of(&request), None);
}

#[tokio::test]
async fn hydration_is_identical_for_every_wire() {
    let Some(h) = Harness::open().await else {
        return;
    };
    for wire in [
        GEMINI,
        Some(WireProtocol::Anthropic),
        Some(WireProtocol::OpenAiChat),
        None,
    ] {
        let cache = h.cache();
        let conv = conv();
        cache.store(&conv, "call_1", "sig-a").await;
        let mut request = request_with(vec![tool_use("call_1", None)]);
        cache.hydrate_request(&conv, &mut request, wire).await;
        assert_eq!(signature_of(&request).as_deref(), Some("sig-a"));
    }
}

#[test]
fn only_signed_tool_use_blocks_make_a_response_worth_caching() {
    let unsigned = response_with(vec![
        CanonicalContent::Text("hi".to_owned()),
        tool_use("call_1", None),
    ]);
    assert_eq!(ThoughtSignatureCache::signed_tool_use_count(&unsigned), 0);

    let signed = response_with(vec![
        CanonicalContent::Text("hi".to_owned()),
        tool_use("call_1", Some("sig-a")),
        tool_use("call_2", None),
        tool_use("call_3", Some("sig-c")),
    ]);
    assert_eq!(ThoughtSignatureCache::signed_tool_use_count(&signed), 2);
}

fn counter_value(name: &str, label: (&str, &str), body: impl FnOnce()) -> Option<u64> {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, body);
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find_map(|(composite, _, _, value)| {
            let key = composite.key();
            let matches_name = key.name() == name;
            let matches_label = key
                .labels()
                .any(|l| l.key() == label.0 && l.value() == label.1);
            match value {
                metrics_util::debugging::DebugValue::Counter(n)
                    if matches_name && matches_label =>
                {
                    Some(n)
                },
                _ => None,
            }
        })
}

// The metrics recorder is thread-local, so DB-backed hydration runs on a
// current-thread runtime inside the recorder scope. The pool is opened on
// that same runtime: a sqlx connection is bound to the reactor that created
// it, and reusing one across runtimes stalls on every acquire.
fn block_on_db(body: impl AsyncFnOnce(Harness)) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            if let Some(h) = Harness::open().await {
                body(h).await;
            }
        });
}

fn db_available() -> bool {
    fixture_database_url().is_ok()
}

#[test]
fn gemini_hydration_records_hit_and_miss() {
    if !db_available() {
        return;
    }
    let hits = counter_value(
        "gateway_signature_hydration_total",
        ("outcome", "hit"),
        || {
            block_on_db(async |h: Harness| {
                let cache = h.cache();
                let conv = conv();
                cache.store(&conv, "call_1", "sig-a").await;
                let mut request = request_with(vec![tool_use("call_1", None)]);
                cache.hydrate_request(&conv, &mut request, GEMINI).await;
            });
        },
    );
    assert_eq!(hits, Some(1));

    let misses = counter_value(
        "gateway_signature_hydration_total",
        ("outcome", "miss"),
        || {
            block_on_db(async |h: Harness| {
                let cache = h.cache();
                let mut request = request_with(vec![tool_use("call_unknown", None)]);
                cache.hydrate_request(&conv(), &mut request, GEMINI).await;
            });
        },
    );
    assert_eq!(misses, Some(1));
}

#[test]
fn non_gemini_hydration_records_nothing() {
    if !db_available() {
        return;
    }
    for wire in [
        Some(WireProtocol::Anthropic),
        Some(WireProtocol::OpenAiChat),
        Some(WireProtocol::OpenAiResponses),
        None,
    ] {
        for outcome in ["hit", "miss"] {
            let recorded = counter_value(
                "gateway_signature_hydration_total",
                ("outcome", outcome),
                || {
                    block_on_db(async |h: Harness| {
                        let cache = h.cache();
                        let conv = conv();
                        cache.store(&conv, "call_1", "sig-a").await;
                        let mut request =
                            request_with(vec![tool_use("call_1", None), tool_use("call_2", None)]);
                        cache.hydrate_request(&conv, &mut request, wire).await;
                    });
                },
            );
            assert_eq!(recorded, None);
        }
    }
}

#[test]
fn uncacheable_response_records_only_when_signatures_are_present() {
    let signed = counter_value(
        "gateway_signature_capture_skipped_total",
        ("reason", "no_conversation_id"),
        || {
            let response = response_with(vec![tool_use("call_1", Some("sig-a"))]);
            ThoughtSignatureCache::note_uncacheable_response(&response, "no_conversation_id");
        },
    );
    assert_eq!(signed, Some(1));

    let unsigned = counter_value(
        "gateway_signature_capture_skipped_total",
        ("reason", "no_conversation_id"),
        || {
            let response = response_with(vec![
                CanonicalContent::Text("hi".to_owned()),
                tool_use("call_1", None),
            ]);
            ThoughtSignatureCache::note_uncacheable_response(&response, "no_conversation_id");
        },
    );
    assert_eq!(unsigned, None);
}

#[tokio::test]
async fn cache_survives_a_poisoned_lock() {
    let Some(h) = Harness::open().await else {
        return;
    };
    let cache = h.cache();
    let conv = conv();
    cache.store(&conv, "call_1", "sig-a").await;

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let poisoned =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.poison_lock())).is_err();
    std::panic::set_hook(previous);
    assert!(poisoned);

    assert_eq!(
        cache.lookup(&conv, "call_1").await.as_deref(),
        Some("sig-a")
    );
    cache.store(&conv, "call_2", "sig-b").await;
    assert_eq!(
        cache.lookup(&conv, "call_2").await.as_deref(),
        Some("sig-b")
    );
}
