use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use systemprompt_extension::brain::ReuseHint;
use systemprompt_gateway::protocol::{CanonicalContent, CanonicalMessage, Role};
use systemprompt_identifiers::ScopeDimension;
use systemprompt_models::attribution::{AttributionEntry, AttributionSource};
use systemprompt_models::origin::{ClientKind, MemoryPreference};
use systemprompt_wire::WireProtocol;

use super::gateway_pipeline::MODEL;
use super::gateway_recall::{FACT_ID, FakeMemoryRecall, Rig, engines, profile, text};

fn client_body() -> Bytes {
    Bytes::from(
        serde_json::to_vec(&serde_json::json!({
            "model": MODEL,
            "max_tokens": 256,
            "system": "be brief",
            "messages": [{"role": "user", "content": "hello gateway"}],
            "temperature": 0.5
        }))
        .expect("encode"),
    )
}

async fn injects_after_the_client_system_prompt(wire: WireProtocol, email: &str) {
    let rig = Rig::new(email).await;
    let fake = Arc::new(FakeMemoryRecall::default());
    let di = rig.inputs(engines(Some(Arc::clone(&fake))), None);
    let run = rig.dispatch(wire, profile("other", true), di).await;

    assert_eq!(fake.calls(), 1, "negative control: the engine was asked");
    assert_eq!(run.status, http::StatusCode::OK);
    let body = text(&run.upstream);
    let block = body.find("<team_memory").expect("block in upstream body");
    let system = body.find("be brief").expect("client system prompt kept");
    assert!(
        system < block,
        "block follows the client system prompt: {body}"
    );
    assert_eq!(
        run.ids_header.as_deref(),
        Some(format!("{FACT_ID}@3").as_str())
    );
    let descriptor = run.memory_recall.expect("descriptor recorded");
    assert!(descriptor.starts_with("injected="), "{descriptor}");
}

#[tokio::test]
async fn recall_injects_into_anthropic_upstream() {
    injects_after_the_client_system_prompt(WireProtocol::Anthropic, "gw-recall-a@example.invalid")
        .await;
}

#[tokio::test]
async fn recall_injects_into_openai_chat_upstream() {
    injects_after_the_client_system_prompt(WireProtocol::OpenAiChat, "gw-recall-c@example.invalid")
        .await;
}

#[tokio::test]
async fn recall_injects_into_responses_upstream() {
    injects_after_the_client_system_prompt(
        WireProtocol::OpenAiResponses,
        "gw-recall-r@example.invalid",
    )
    .await;
}

#[tokio::test]
async fn recall_turns_passthrough_off_when_memory_is_injected() {
    let rig = Rig::new("gw-recall-pass@example.invalid").await;
    let fake = Arc::new(FakeMemoryRecall::default());
    let di = rig.inputs(engines(Some(fake)), Some(client_body()));
    let run = rig
        .dispatch(WireProtocol::Anthropic, profile("other", true), di)
        .await;
    assert_ne!(run.upstream, client_body());
    assert!(text(&run.upstream).contains("<team_memory"));
}

#[tokio::test]
async fn recall_absent_keeps_the_raw_body_byte_identical() {
    let mut rig = Rig::new("gw-recall-absent@example.invalid").await;
    rig.automatic_caching = false;
    let di = rig.inputs(engines(None), Some(client_body()));
    let run = rig
        .dispatch(WireProtocol::Anthropic, profile("other", true), di)
        .await;
    assert_eq!(run.upstream, client_body(), "passthrough is untouched");
    assert_eq!(run.ids_header, None);
}

#[tokio::test]
async fn recall_past_its_deadline_is_skipped() {
    let rig = Rig::new("gw-recall-deadline@example.invalid").await;
    let fake = Arc::new(FakeMemoryRecall::slow(Duration::from_millis(200)));
    let di = rig.inputs(engines(Some(Arc::clone(&fake))), None);
    let mut slow = profile("other", true);
    slow.deadline_ms = 50;
    let run = rig.dispatch(WireProtocol::Anthropic, slow, di).await;
    assert_eq!(run.status, http::StatusCode::OK);
    assert_eq!(fake.calls(), 1);
    assert!(!text(&run.upstream).contains("<team_memory"));
    assert_eq!(run.ids_header, None);
    assert_eq!(run.memory_recall.as_deref(), Some("skipped=deadline"));
}

#[tokio::test]
async fn recall_header_off_never_asks_the_engine() {
    let rig = Rig::new("gw-recall-off@example.invalid").await;
    let fake = Arc::new(FakeMemoryRecall::default());
    let mut di = rig.inputs(engines(Some(Arc::clone(&fake))), None);
    di.ctx.memory = MemoryPreference::Off;
    let run = rig
        .dispatch(WireProtocol::Anthropic, profile("other", true), di)
        .await;
    assert_eq!(fake.calls(), 0);
    assert_eq!(run.memory_recall.as_deref(), Some("skipped=header_off"));
    assert!(!text(&run.upstream).contains("<team_memory"));
}

#[tokio::test]
async fn recall_on_a_tool_round_trip_asks_for_the_same_turn() {
    let rig = Rig::new("gw-recall-tool@example.invalid").await;
    let fake = Arc::new(FakeMemoryRecall::default());
    let mut di = rig.inputs(engines(Some(Arc::clone(&fake))), None);
    di.request.messages.push(CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::ToolUse {
            id: "toolu_1".to_owned(),
            name: "lookup".to_owned(),
            input: serde_json::json!({"q": "acme"}),
            signature: None,
            cache_control: None,
        }],
    });
    di.request.messages.push(CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::ToolResult {
            tool_use_id: "toolu_1".to_owned(),
            content: vec![CanonicalContent::text("1200 EUR".to_owned())],
            is_error: false,
            structured_content: None,
            meta: None,
            cache_control: None,
        }],
    });
    rig.dispatch(WireProtocol::Anthropic, profile("other", true), di)
        .await;
    assert_eq!(fake.last().expect("asked").reuse, ReuseHint::SameTurn);
}

#[tokio::test]
async fn recall_for_a_client_not_enabled_is_skipped() {
    let rig = Rig::new("gw-recall-client@example.invalid").await;
    let fake = Arc::new(FakeMemoryRecall::default());
    let mut di = rig.inputs(engines(Some(Arc::clone(&fake))), None);
    di.ctx.origin.client = ClientKind::ClaudeCode;
    let run = rig
        .dispatch(WireProtocol::Anthropic, profile("claude-code", false), di)
        .await;
    assert_eq!(fake.calls(), 0);
    assert_eq!(
        run.memory_recall.as_deref(),
        Some("skipped=client_not_enabled")
    );
}

#[tokio::test]
async fn recall_carries_the_project_attribution() {
    let rig = Rig::new("gw-recall-project@example.invalid").await;
    let fake = Arc::new(FakeMemoryRecall::default());
    let mut di = rig.inputs(engines(Some(Arc::clone(&fake))), None);
    di.ctx.attribution.entries.push(AttributionEntry {
        dimension: ScopeDimension::try_new("project").expect("dimension"),
        value: "proj-x".to_owned(),
        source: AttributionSource::Header,
    });
    rig.dispatch(WireProtocol::Anthropic, profile("other", true), di)
        .await;
    let asked = fake.last().expect("asked");
    assert_eq!(asked.principal.project.as_deref(), Some("proj-x"));
    assert_eq!(asked.reuse, ReuseHint::NewTurn);
}
