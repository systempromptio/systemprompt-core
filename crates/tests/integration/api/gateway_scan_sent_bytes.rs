//! The request safety scan judges the bytes that go upstream. A blocking
//! scanner sees content the inbound parser does not model (`document`,
//! `search_result`, unknown block types, OpenAI `file` parts) on every inbound
//! wire, an inspection surface cut short by its budget refuses the request,
//! and no upstream ever receives a byte the scanner did not read.

use std::sync::Arc;

use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_database::DbPool;
use systemprompt_gateway::protocol::InboundAdapter;
use systemprompt_gateway::protocol::canonical::{CanonicalRequest, CanonicalResponse};
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_gateway::service::{DispatchError, GatewayError, GatewayService};
use systemprompt_gateway::{
    Finding, PHASE_REQUEST, SafetyScanner, ScanError, ScannerSettings, Severity,
    register_safety_scanner,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_wire::WireProtocol;
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, gateway_config, gw_repos, inputs_with, install_provider_api_key,
    provider_registry,
};

const DLP: &str = "test_sent_bytes_dlp";
const NEEDLE: &str = "dlp-needle-7Qx2";
const CATEGORY: &str = "dlp_secret";

struct NeedleScanner;

#[async_trait::async_trait]
impl SafetyScanner for NeedleScanner {
    fn name(&self) -> &'static str {
        DLP
    }

    async fn scan_request(&self, req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        let hit = [false, true]
            .into_iter()
            .flat_map(|history| req.safety_parts(history))
            .any(|(_, text)| text.contains(NEEDLE));
        Ok(hit
            .then(|| Finding {
                phase: PHASE_REQUEST,
                severity: Severity::High,
                category: CATEGORY.to_owned(),
                excerpt: None,
                scanner: DLP,
                spans: Vec::new(),
                replacement: None,
            })
            .into_iter()
            .collect())
    }

    async fn scan_response_final(
        &self,
        _response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
}

register_safety_scanner!(
    |_: &dyn systemprompt_extension::ExtensionContext, _: &ScannerSettings| NeedleScanner,
    name = DLP
);

async fn install_blocking_policy(pool: &DbPool) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) VALUES ($1, $2, $3, \
         TRUE, 100)",
    )
    .bind(format!("gwpol_{}", Uuid::new_v4().simple()))
    .bind(format!("gw-sent-bytes-{}", Uuid::new_v4().simple()))
    .bind(json!({ "safety": { "scanners": [DLP], "block_categories": [CATEGORY] } }))
    .execute(pool.pool().as_ref())
    .await?;
    Ok(())
}

fn anthropic_reply() -> Value {
    json!({
        "id": "msg_sent", "type": "message", "role": "assistant", "model": MODEL,
        "content": [{"type": "text", "text": "ok"}], "stop_reason": "end_turn",
        "usage": {"input_tokens": 3, "output_tokens": 2}
    })
}

fn chat_reply() -> Value {
    json!({
        "id": "chatcmpl_sent", "object": "chat.completion", "model": MODEL,
        "choices": [{"index": 0, "finish_reason": "stop",
                     "message": {"role": "assistant", "content": "ok"}}],
        "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}
    })
}

struct Case {
    inbound: Arc<dyn InboundAdapter>,
    upstream_wire: WireProtocol,
    family: ClientFamily,
    reply: Value,
    body: Value,
}

struct Outcome {
    result: Result<(), DispatchError>,
    upstream_bodies: Vec<Vec<u8>>,
}

async fn dispatch(case: Case, label: &str) -> anyhow::Result<Outcome> {
    install_provider_api_key();
    let _ = setup_ctx().await?;
    let database = systemprompt_test_fixtures::DisposableDb::with_schema(label).await;
    let pool = database.test_pool().await;
    let cred = seed_admin_credential(
        &pool,
        &format!("{label}-{}@example.invalid", Uuid::new_v4().simple()),
    )
    .await?;
    install_blocking_policy(&pool).await?;
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(case.reply))
        .mount(&upstream)
        .await;
    let registry = provider_registry(&upstream.uri(), PROVIDER, case.upstream_wire, case.family);
    let raw = Bytes::from(serde_json::to_vec(&case.body)?);
    let request = case.inbound.parse_request(&raw)?;
    let di = inputs_with(&cred, request, false, case.inbound, raw);
    let repos = gw_repos(&pool);
    let result = GatewayService::dispatch(&gateway_config(PROVIDER), &registry, &pool, &repos, di)
        .await
        .map(|_| ());
    let upstream_bodies = upstream
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.body)
        .collect();
    let _ = repos
        .background
        .drain(std::time::Duration::from_secs(30))
        .await;
    database.drop_now().await;
    Ok(Outcome {
        result,
        upstream_bodies,
    })
}

fn assert_never_forwarded_unscanned(outcome: &Outcome) {
    for body in &outcome.upstream_bodies {
        assert!(
            !String::from_utf8_lossy(body).contains(NEEDLE),
            "an upstream received the needle the scanner was meant to block"
        );
    }
}

fn assert_safety_blocked(outcome: &Outcome) {
    assert_never_forwarded_unscanned(outcome);
    match &outcome.result {
        Err(DispatchError::Recorded(GatewayError::Safety(blocked))) => {
            assert_eq!(blocked.category, CATEGORY);
        },
        other => panic!("expected a safety block, got {other:?}"),
    }
}

fn anthropic_case(block: Value) -> Case {
    Case {
        inbound: Arc::new(AnthropicMessagesInbound),
        upstream_wire: WireProtocol::Anthropic,
        family: ClientFamily::Claude,
        reply: anthropic_reply(),
        body: json!({
            "model": MODEL,
            "max_tokens": 64,
            "messages": [{"role": "user", "content": [
                block,
                {"type": "text", "text": "summarise"}
            ]}]
        }),
    }
}

#[tokio::test]
async fn a_document_block_is_scanned_on_the_anthropic_lane() -> anyhow::Result<()> {
    let block = json!({"type": "document", "source": {
        "type": "text", "media_type": "text/plain", "data": format!("customer key {NEEDLE}")
    }});
    assert_safety_blocked(&dispatch(anthropic_case(block), "gw_sent_doc").await?);
    Ok(())
}

#[tokio::test]
async fn a_search_result_block_is_scanned_on_the_anthropic_lane() -> anyhow::Result<()> {
    let block = json!({"type": "search_result", "source": "https://x.invalid", "title": "t",
        "content": [{"type": "text", "text": NEEDLE}]});
    assert_safety_blocked(&dispatch(anthropic_case(block), "gw_sent_search").await?);
    Ok(())
}

#[tokio::test]
async fn an_unknown_block_is_scanned_on_the_anthropic_lane() -> anyhow::Result<()> {
    let block = json!({"type": "future_block", "payload": {"nested": [NEEDLE]}});
    assert_safety_blocked(&dispatch(anthropic_case(block), "gw_sent_unknown").await?);
    Ok(())
}

#[tokio::test]
async fn an_unmodelled_top_level_field_is_scanned_on_the_anthropic_lane() -> anyhow::Result<()> {
    let mut case = anthropic_case(json!({"type": "text", "text": "hi"}));
    case.body["container"] = json!({"note": NEEDLE});
    assert_safety_blocked(&dispatch(case, "gw_sent_field").await?);
    Ok(())
}

#[tokio::test]
async fn a_truncated_inspection_surface_refuses_the_request() -> anyhow::Result<()> {
    let filler: Vec<Value> = (0..600_000).map(|_| json!("a")).collect();
    let mut items = filler;
    items.push(json!(NEEDLE));
    let block = json!({"type": "future_block", "items": items});
    let outcome = dispatch(anthropic_case(block), "gw_sent_trunc").await?;
    assert_never_forwarded_unscanned(&outcome);
    assert!(
        outcome.upstream_bodies.is_empty(),
        "an incompletely inspected request must not reach the upstream"
    );
    match &outcome.result {
        Err(DispatchError::Recorded(GatewayError::Safety(blocked))) => {
            assert_eq!(blocked.category, "inspection_incomplete");
        },
        other => panic!("expected an incomplete-inspection refusal, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn a_file_part_text_is_scanned_on_the_openai_chat_lane() -> anyhow::Result<()> {
    let case = Case {
        inbound: Arc::new(OpenAiChatInbound),
        upstream_wire: WireProtocol::OpenAiChat,
        family: ClientFamily::OpenAi,
        reply: chat_reply(),
        body: json!({
            "model": MODEL,
            "max_tokens": 64,
            "messages": [{"role": "user", "content": [
                {"type": "file", "file": {"filename": format!("{NEEDLE}.txt"), "file_data": "UEsDBA=="}},
                {"type": "text", "text": "summarise"}
            ]}]
        }),
    };
    assert_safety_blocked(&dispatch(case, "gw_sent_chat").await?);
    Ok(())
}

#[tokio::test]
async fn an_input_file_part_on_the_responses_wire_never_reaches_the_upstream_unscanned()
-> anyhow::Result<()> {
    let case = Case {
        inbound: Arc::new(OpenAiResponsesInbound),
        upstream_wire: WireProtocol::Anthropic,
        family: ClientFamily::Claude,
        reply: anthropic_reply(),
        body: json!({
            "model": MODEL,
            "max_output_tokens": 64,
            "input": [{"role": "user", "content": [
                {"type": "input_file", "filename": "a.txt", "file_data": NEEDLE},
                {"type": "input_text", "text": "summarise"}
            ]}]
        }),
    };
    let outcome = dispatch(case, "gw_sent_resp").await?;
    assert_never_forwarded_unscanned(&outcome);
    match &outcome.result {
        Err(DispatchError::Recorded(GatewayError::Safety(_))) | Ok(()) => {},
        other => panic!("expected a block or a re-encoded send, got {other:?}"),
    }
    Ok(())
}

fn assert_forwarded(outcome: &Outcome) {
    assert!(
        outcome.result.is_ok(),
        "an inspectable request must be forwarded, got {:?}",
        outcome.result
    );
    assert_eq!(outcome.upstream_bodies.len(), 1);
}

fn session(messages: Vec<Value>) -> Case {
    let mut case = anthropic_case(json!({"type": "text", "text": "hi"}));
    case.body["messages"] = Value::Array(messages);
    case
}

fn screenshot(bytes: usize) -> Value {
    json!({"type": "image", "source": {
        "type": "base64", "media_type": "image/png", "data": "Q".repeat(bytes)
    }})
}

#[tokio::test]
async fn an_enforcing_policy_forwards_a_five_megabyte_screenshot() -> anyhow::Result<()> {
    let outcome = dispatch(anthropic_case(screenshot(5 * 1024 * 1024)), "gw_sent_shot").await?;
    assert_forwarded(&outcome);
    Ok(())
}

#[tokio::test]
async fn an_enforcing_policy_forwards_a_two_hundred_turn_text_session() -> anyhow::Result<()> {
    let messages = (0..200)
        .map(|turn| {
            let role = if turn % 2 == 0 { "user" } else { "assistant" };
            json!({"role": role, "content": [
                {"type": "text", "text": format!("turn {turn} {}", "w".repeat(20_000))}
            ]})
        })
        .chain(std::iter::once(
            json!({"role": "user", "content": "continue"}),
        ))
        .collect();
    assert_forwarded(&dispatch(session(messages), "gw_sent_turns").await?);
    Ok(())
}

#[tokio::test]
async fn an_enforcing_policy_forwards_a_long_tool_use_history() -> anyhow::Result<()> {
    let mut messages = vec![json!({"role": "user", "content": "start"})];
    for call in 0..3_000 {
        let id = format!("toolu_{call:05}");
        messages.push(json!({"role": "assistant", "content": [{
            "type": "tool_use", "id": id, "name": "read_file",
            "input": {"path": format!("src/f{call}.rs"), "start": 1, "mode": "full"}
        }]}));
        messages.push(json!({"role": "user", "content": [{
            "type": "tool_result", "tool_use_id": id,
            "content": [{"type": "text", "text": "fn main() {}"}]
        }]}));
    }
    assert_forwarded(&dispatch(session(messages), "gw_sent_tools").await?);
    Ok(())
}

#[tokio::test]
async fn a_blocking_scanner_sees_every_text_leaf_beside_large_images() -> anyhow::Result<()> {
    let mut case = anthropic_case(screenshot(3 * 1024 * 1024));
    case.body["messages"][0]["content"] = json!([
        screenshot(3 * 1024 * 1024),
        {"type": "text", "text": "w".repeat(3 * 1024 * 1024)},
        screenshot(3 * 1024 * 1024),
        {"type": "text", "text": format!("tail {NEEDLE}")}
    ]);
    assert_safety_blocked(&dispatch(case, "gw_sent_mixed").await?);
    Ok(())
}
