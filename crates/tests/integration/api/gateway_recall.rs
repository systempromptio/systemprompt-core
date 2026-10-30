//! Team-memory recall stage driven through `GatewayService::dispatch`
//! against a wiremock upstream with a fake `MemoryRecall` engine.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::to_bytes;
use bytes::Bytes;
use chrono::Utc;
use serde_json::{Value, json};
use systemprompt_database::DbPool;
use systemprompt_extension::brain::{
    InjectedFact, MemoryRecall, RecallBlock, RecallError, RecallOutcome, RecallReceipt,
    RecallRequest,
};
use systemprompt_gateway::protocol::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::service::GatewayService;
use systemprompt_gateway::{DispatchInputs, GatewayEngines};
use systemprompt_identifiers::{AiRequestId, BrainFactId};
use systemprompt_manifest::profile::BrainRecallProfile;
use systemprompt_models::origin::ClientKind;
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::{AuthedFixture, seed_admin_credential};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, buffered_response_json, canonical_request, gateway_config, gw_repos,
    inputs_with, install_provider_api_key, provider_registry,
};

pub(super) const FACT_ID: &str = "m_0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";

#[derive(Default)]
pub(super) struct FakeMemoryRecall {
    calls: AtomicUsize,
    last: Mutex<Option<RecallRequest>>,
    delay: Option<Duration>,
}

impl FakeMemoryRecall {
    pub(super) fn slow(delay: Duration) -> Self {
        Self {
            delay: Some(delay),
            ..Self::default()
        }
    }

    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    pub(super) fn last(&self) -> Option<RecallRequest> {
        self.last.lock().expect("lock").clone()
    }
}

#[async_trait::async_trait]
impl MemoryRecall for FakeMemoryRecall {
    fn name(&self) -> &'static str {
        "fake"
    }

    async fn recall(&self, request: &RecallRequest) -> Result<RecallOutcome, RecallError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().expect("lock") = Some(request.clone());
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        let block = RecallBlock::new("org", Utc::now())
            .with_fact(InjectedFact {
                id: BrainFactId::try_new(FACT_ID).expect("fact id"),
                revision: 3,
                source: "doc:pricing".to_owned(),
                text: "acme costs 1200 EUR".to_owned(),
            })
            .with_tokens(12);
        Ok(RecallOutcome::Injected(block))
    }

    async fn record(&self, _receipt: &RecallReceipt) -> Result<(), RecallError> {
        Ok(())
    }
}

pub(super) fn profile(client: &str, enabled: bool) -> BrainRecallProfile {
    BrainRecallProfile {
        clients: [(client.to_owned(), enabled)].into_iter().collect(),
        deadline_ms: 2000,
        ..BrainRecallProfile::default()
    }
}

pub(super) fn engines(recall: Option<Arc<FakeMemoryRecall>>) -> Arc<GatewayEngines> {
    let engines = GatewayEngines::new(
        Default::default(),
        &systemprompt_test_fixtures::offline_extension_context(),
        systemprompt_test_fixtures::ensure_test_secrets_bootstrap(),
    )
    .expect("gateway engines build");
    Arc::new(match recall {
        Some(fake) => engines.with_memory_recall(fake),
        None => engines,
    })
}

pub(super) fn upstream_path(wire: WireProtocol) -> &'static str {
    match wire {
        WireProtocol::OpenAiChat => "/chat/completions",
        WireProtocol::OpenAiResponses => "/responses",
        _ => "/messages",
    }
}

fn reply(wire: WireProtocol) -> Value {
    match wire {
        WireProtocol::OpenAiChat => json!({
            "id": "chatcmpl_recall", "object": "chat.completion", "model": MODEL,
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "hi"},
                         "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 11, "completion_tokens": 7, "total_tokens": 18}
        }),
        WireProtocol::OpenAiResponses => json!({
            "id": "resp_recall", "object": "response", "status": "completed", "model": MODEL,
            "output": [{"type": "message", "id": "msg_1", "role": "assistant",
                        "status": "completed",
                        "content": [{"type": "output_text", "text": "hi", "annotations": []}]}],
            "usage": {"input_tokens": 11, "output_tokens": 7, "total_tokens": 18}
        }),
        _ => buffered_response_json(),
    }
}

pub(super) struct Run {
    pub status: http::StatusCode,
    pub ids_header: Option<String>,
    pub upstream: Bytes,
    pub memory_recall: Option<String>,
}

pub(super) struct Rig {
    pub pool: DbPool,
    pub cred: AuthedFixture,
    pub automatic_caching: bool,
}

impl Rig {
    pub(super) async fn new(email: &str) -> Self {
        install_provider_api_key();
        let (pool, _ctx) = setup_ctx().await.expect("setup");
        let cred = seed_admin_credential(&pool, email)
            .await
            .expect("credential");
        Self {
            pool,
            cred,
            automatic_caching: true,
        }
    }

    pub(super) fn inputs(
        &self,
        engines: Arc<GatewayEngines>,
        raw: Option<Bytes>,
    ) -> DispatchInputs {
        let inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
        let mut di = inputs_with(
            &self.cred,
            canonical_request(MODEL, false),
            false,
            inbound,
            raw.unwrap_or_default(),
        );
        di.ctx.origin.client = ClientKind::Other;
        di.engines = engines;
        di
    }

    pub(super) async fn dispatch(
        &self,
        wire: WireProtocol,
        profile: BrainRecallProfile,
        di: DispatchInputs,
    ) -> Run {
        let upstream = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(upstream_path(wire)))
            .respond_with(ResponseTemplate::new(200).set_body_json(reply(wire)))
            .mount(&upstream)
            .await;
        let family = match wire {
            WireProtocol::Anthropic => ClientFamily::Claude,
            _ => ClientFamily::OpenAi,
        };
        let registry = provider_registry(&upstream.uri(), PROVIDER, wire, family);
        let repos = gw_repos(&self.pool).with_brain_recall(profile);
        let id: AiRequestId = di.ctx.ai_request_id.clone();
        let mut config = gateway_config(PROVIDER);
        config.automatic_prompt_caching = self.automatic_caching;
        let resp = GatewayService::dispatch(&config, &registry, &self.pool, &repos, di)
            .await
            .expect("dispatch succeeds");
        let status = resp.status();
        let ids_header = resp
            .headers()
            .get("x-systemprompt-memory-ids")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        to_bytes(resp.into_body(), 1024 * 1024).await.expect("body");
        assert_eq!(
            repos.background.drain(Duration::from_secs(30)).await,
            systemprompt_traits::DrainOutcome::Drained
        );
        let memory_recall: Option<String> =
            sqlx::query_scalar("SELECT memory_recall FROM ai_requests WHERE id = $1")
                .bind(id.as_str())
                .fetch_one(self.pool.pool().as_ref())
                .await
                .expect("ai_requests row");
        let received = upstream.received_requests().await.expect("recorded");
        assert_eq!(received.len(), 1, "one upstream call");
        Run {
            status,
            ids_header,
            upstream: Bytes::from(received[0].body.clone()),
            memory_recall,
        }
    }
}

pub(super) fn text(bytes: &Bytes) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}
