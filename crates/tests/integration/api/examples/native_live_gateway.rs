//! Private pipe gateway fixture: real execution capabilities and budget
//! settlement for native requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, ensure};
use axum::body::to_bytes;
use bytes::Bytes;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_api::services::gateway::service::GatewayService;
use systemprompt_api::services::gateway::{
    DispatchInputs, GatewayAudit, GatewayRepositories, GatewayRequestContext,
};
use systemprompt_identifiers::{
    AiRequestId, ContextId, ModelId, ProviderId, RouteId, SecretName, TraceId,
};
use systemprompt_models::services::{
    ApiSurface, GatewayConfig, GatewayRoute, ModelPricing, ProviderEntry, ProviderModel,
    ProviderRegistry, WireProtocol,
};
use systemprompt_security::policy::types::AccessScope;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/native_live_setup.rs"]
mod native_live_setup;
use systemprompt_evaluation::repository::experiments::ExecutionCapabilityRepository;
use systemprompt_models::wire::origin::{ClientKind, InboundWireProtocol, RequestOrigin};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
fn wire(
    route: &str,
) -> Result<(
    WireProtocol,
    ApiSurface,
    Arc<dyn InboundAdapter>,
    &'static str,
)> {
    Ok(match route {
        "/v1/messages" => (
            WireProtocol::Anthropic,
            ApiSurface::Anthropic,
            Arc::new(AnthropicMessagesInbound),
            "/messages",
        ),
        "/v1/chat/completions" => (
            WireProtocol::OpenAiChat,
            ApiSurface::OpenAi,
            Arc::new(OpenAiChatInbound),
            "/chat/completions",
        ),
        "/v1/responses" => (
            WireProtocol::OpenAiResponses,
            ApiSurface::OpenAi,
            Arc::new(OpenAiResponsesInbound),
            "/responses",
        ),
        _ => anyhow::bail!("Unsupported retained provider route"),
    })
}
fn config() -> GatewayConfig {
    let mut route = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: "*".to_owned(),
        provider: ProviderId::new("native-fixture"),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
    };
    route.ensure_id();
    GatewayConfig {
        enabled: true,
        routes: vec![route],
        ..GatewayConfig::default()
    }
}
fn registry(
    endpoint: &str,
    model: &str,
    wire: WireProtocol,
    surface: ApiSurface,
    priced: bool,
) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("native-fixture"),
            wire,
            surface,
            endpoint: endpoint.to_owned(),
            api_key_secret: SecretName::new("anthropic"),
            governance: Default::default(),
            extra_headers: HashMap::new(),
            models: if priced {
                vec![ProviderModel {
                    id: ModelId::new(model),
                    aliases: vec![],
                    governance: None,
                    upstream_model: None,
                    pricing: ModelPricing {
                        input_per_million: 1.0,
                        output_per_million: 2.0,
                        ..ModelPricing::default()
                    },
                    capabilities: Default::default(),
                    limits: Default::default(),
                }]
            } else {
                vec![]
            },
        }],
    }
}

async fn summary(harness: &native_live_setup::Harness) -> Result<Value> {
    let requests:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('request',to_jsonb(r),'reservation',to_jsonb(b)) FROM ai_requests r JOIN eval_request_reservations m ON m.request_id=r.id JOIN eval_budget_reservations b ON b.id=m.reservation_id WHERE m.execution_id IN (SELECT id FROM eval_executions WHERE experiment_id=$1) ORDER BY r.created_at,r.id").bind(harness.experiment.as_str()).fetch_all(&harness.pg).await?;
    let budget:Value=sqlx::query_scalar("SELECT to_jsonb(a) FROM eval_budget_accounts a JOIN eval_experiments e ON e.budget_id=a.id WHERE e.id=$1").bind(harness.experiment.as_str()).fetch_one(&harness.pg).await?;
    Ok(json!({"requests":requests,"budget":budget}))
}
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 1,
        "usage: native_live_gateway absolute-plan-file"
    );
    let plan_path = Path::new(&args[0]);
    ensure!(
        plan_path.is_absolute() && std::fs::metadata(plan_path)?.len() < 1024 * 1024,
        "Invalid bounded native plan"
    );
    let plan: Value = serde_json::from_slice(&std::fs::read(plan_path)?)?;
    let harness = native_live_setup::Harness::start(&plan).await?;
    let outcome = serve(&harness).await;
    let cleanup = harness
        .workers()
        .revoke(&harness.owner, &harness.worker.id)
        .await;
    match (outcome, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(primary), Err(cleanup)) => Err(anyhow::anyhow!(
            "{primary:#}; worker revocation also failed: {cleanup}"
        )),
    }
}
async fn serve(harness: &native_live_setup::Harness) -> Result<()> {
    let (_, lease) = harness.claimed_lease().await?;
    let capabilities = harness
        .repositories(native_live_setup::fixture_admission())?
        .capabilities;
    let access = capabilities.issue(&harness.owner, &lease).await?;
    let bootstrap = ensure_test_bootstrap();
    let _ctx = fixture_app_context(&harness.pool, &bootstrap.database_url)?;
    let journal = systemprompt_api::services::gateway::audit::journal::GatewayJournal::open(
        systemprompt_config::ProfileBootstrap::get_path()?,
        systemprompt_config::SecretsBootstrap::get()?,
    )?;
    let mut repos = GatewayRepositories::new(
        &harness.pool,
        journal,
        Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(&harness.pool)?,
        )),
    )?;
    repos.evaluations = harness
        .repositories(native_live_setup::fixture_admission())?
        .gateway;
    let mut output = tokio::io::stdout();
    output.write_all(format!("{}\n",json!({"kind":"ready","execution_token":access.expose_token(),"session_id":access.session_id,"execution_id":lease.execution_id,"initial":summary(harness).await?,"fixture_only":true,"automated_target_enabled":false})).as_bytes()).await?;
    output.flush().await?;
    let mut input = tokio::io::BufReader::new(tokio::io::stdin());
    let mut count = 0usize;
    loop {
        let mut line = String::new();
        let size = tokio::time::timeout(
            Duration::from_secs(90),
            (&mut input).take(20 * 1024 * 1024 + 1).read_line(&mut line),
        )
        .await
        .context("Native fixture pipe idle deadline")??;
        if size == 0 {
            break;
        }
        ensure!(size <= 20 * 1024 * 1024, "Native fixture RPC exceeds bound");
        count += 1;
        ensure!(count <= 64, "Native fixture RPC count exceeded");
        let message: Value = serde_json::from_str(&line)?;
        let result = if message["kind"] == "finalize" {
            let accounting_failure =
                verify_accounting_failure(harness, &repos, &access.session_id).await?;
            let before = summary(harness).await?;
            sqlx::query("UPDATE eval_executions SET lease_expires_at=NOW()-INTERVAL '1 second' WHERE id=$1 AND lease_owner=$2 AND fencing_token=$3").bind(lease.execution_id.as_str()).bind(lease.worker_id.as_str()).bind(lease.fencing_token).execute(&harness.pg).await?;
            let stale_denied = capabilities
                .authenticate(access.expose_token(), &harness.environment)
                .await
                .is_err();
            sqlx::query("UPDATE eval_executions SET lease_expires_at=NOW()+INTERVAL '60 seconds' WHERE id=$1 AND lease_owner=$2 AND fencing_token=$3").bind(lease.execution_id.as_str()).bind(lease.worker_id.as_str()).bind(lease.fencing_token).execute(&harness.pg).await?;
            let replacement = capabilities.issue(&harness.owner, &lease).await?;
            let revoked_denied = capabilities
                .authenticate(access.expose_token(), &harness.environment)
                .await
                .is_err();
            ensure!(
                capabilities
                    .authenticate(replacement.expose_token(), &harness.environment)
                    .await
                    .is_ok(),
                "fresh execution capability rejected"
            );
            harness
                .workers()
                .revoke(&harness.owner, &harness.worker.id)
                .await?;
            let after = summary(harness).await?;
            ensure!(
                before == after,
                "authentication probes changed budget or requests"
            );
            ensure!(
                stale_denied && revoked_denied,
                "stale or revoked capability admitted"
            );
            json!({"kind":"finalized","stale_lease_denied":stale_denied,"revoked_token_denied":revoked_denied,"snapshot":after,"accounting_failure":accounting_failure,"automated_target_enabled":false})
        } else if message["kind"] == "authenticate" {
            match capabilities
                .authenticate(
                    message["credential"].as_str().unwrap_or(""),
                    &harness.environment,
                )
                .await
            {
                Ok(principal)
                    if message["session_id"].as_str() == Some(principal.session_id.as_str()) =>
                {
                    json!({"kind":"authenticated","execution_id":principal.identity.execution_id})
                },
                _ => json!({"kind":"rejected","status":401}),
            }
        } else {
            let before = summary(harness).await?;
            let dispatch = tokio::time::timeout(
                Duration::from_secs(25),
                dispatch(harness, &repos, &capabilities, &lease, &message),
            )
            .await
            .context("Native gateway dispatch deadline");
            match dispatch {
                Ok(Ok(reply)) => reply,
                other => {
                    json!({"kind":"rejected","status":403,"diagnostic":format!("{other:?}"),"before":before,"after":summary(harness).await?})
                },
            }
        };
        output.write_all(format!("{}\n", result).as_bytes()).await?;
        output.flush().await?;
        if message["kind"] == "finalize" {
            break;
        }
    }
    harness
        .workers()
        .revoke(&harness.owner, &harness.worker.id)
        .await?;
    Ok(())
}
async fn dispatch(
    harness: &native_live_setup::Harness,
    repos: &GatewayRepositories,
    capabilities: &ExecutionCapabilityRepository,
    lease: &systemprompt_evaluation::repository::experiments::ExecutionLease,
    message: &Value,
) -> Result<Value> {
    let token = message["credential"]
        .as_str()
        .context("missing execution capability")?;
    let principal = capabilities
        .authenticate(token, &harness.environment)
        .await?;
    ensure!(
        message["session_id"].as_str() == Some(principal.session_id.as_str()),
        "Forged execution session"
    );
    harness
        .experiments()
        .heartbeat(&harness.owner, lease)
        .await?;
    let (outbound, surface, inbound, upstream_path) =
        wire(message["path"].as_str().context("Missing native route")?)?;
    let raw = Bytes::from(serde_json::to_vec(&message["body"])?);
    ensure!(raw.len() <= 512000, "Native request exceeds bound");
    let request = inbound
        .parse_request(&raw)
        .map_err(|error| anyhow::anyhow!("native inbound: {error}"))?;
    let upstream = MockServer::start().await;
    let status = message["reply"]["status"]
        .as_u64()
        .context("Missing fixture status")?;
    ensure!((100..=599).contains(&status), "Invalid fixture status");
    let mut response = ResponseTemplate::new(status as u16).set_body_raw(
        message["reply"]["body"]
            .as_str()
            .context("Missing fixture wire")?,
        message["reply"]["content_type"]
            .as_str()
            .unwrap_or("application/json"),
    );
    if message["reply"]["delay"] == true {
        response = response.set_delay(Duration::from_secs(60));
    }
    Mock::given(method("POST"))
        .and(path(upstream_path))
        .respond_with(response)
        .mount(&upstream)
        .await;
    let model = harness.native_model.as_str();
    let configured = config();
    let mut providers = registry(
        &upstream.uri(),
        model,
        outbound,
        surface,
        message["proof_probe"] != "unknown-pricing",
    );
    if message["proof_probe"] == "cost-bound" {
        for provider in &mut providers.providers {
            for model in &mut provider.models {
                model.pricing.input_per_million = 1_000_000_000.0;
                model.pricing.output_per_million = 1_000_000_000.0;
            }
        }
    }
    let id = AiRequestId::generate();
    let ctx = GatewayRequestContext {
        ai_request_id: id.clone(),
        user_id: principal.identity.owner_id,
        session_id: Some(principal.session_id),
        context_id: ContextId::generate(),
        gateway_conversation_id: None,
        client_session_id: None,
        trace_id: Some(TraceId::generate()),
        access_scope: AccessScope::Unknown,
        client_id: None,
        provider: "native-fixture".to_owned(),
        requested_model: Some(request.model.as_str().to_owned()),
        model: request.model.as_str().to_owned(),
        max_tokens: Some(request.max_tokens),
        is_streaming: request.stream,
        origin: RequestOrigin::gateway(ClientKind::Other, inbound.wire()),
        access_log: None,
    };
    let response = GatewayService::dispatch(
        &configured,
        &providers,
        &harness.pool,
        repos,
        DispatchInputs {
            request,
            raw_body: raw,
            ctx,
            inbound,
            forward_headers: vec![],
            identity_headers: vec![],
            governance: systemprompt_test_fixtures::default_governance_engine(),
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("native gateway: {error:?}"))?;
    let (parts, body) = response.into_parts();
    let bytes =
        tokio::time::timeout(Duration::from_secs(10), to_bytes(body, 16 * 1024 * 1024)).await??;
    let mut snapshot = summary(harness).await?;
    for _ in 0..100 {
        if snapshot["requests"].as_array().is_some_and(|requests| {
            requests.iter().any(|row| {
                row["request"]["id"] == id.as_str()
                    && row["reservation"]["actual"].as_i64().is_some()
            })
        }) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
        snapshot = summary(harness).await?;
    }
    let recorded = snapshot["requests"]
        .as_array()
        .and_then(|requests| {
            requests
                .iter()
                .find(|row| row["request"]["id"] == id.as_str())
        })
        .context("Native attempt lacked execution reservation")?;
    if parts.status.is_success() {
        ensure!(
            recorded["request"]["status"] == "completed"
                && recorded["request"]["input_tokens"] == 11
                && recorded["request"]["output_tokens"] == 7
                && recorded["request"]["cost_microdollars"] == 25
                && recorded["reservation"]["actual"] == 25,
            "Native successful response lacked exact wire usage and atomic accounting"
        );
    } else {
        ensure!(
            recorded["request"]["status"] == "failed",
            "Native provider failure lacked failed audit evidence"
        );
    }
    Ok(
        json!({"kind":"response","status":parts.status.as_u16(),"content_type":parts.headers.get("content-type").and_then(|value|value.to_str().ok()),"body":String::from_utf8(bytes.to_vec())?,"request_id":id.as_str(),"snapshot":snapshot}),
    )
}

async fn verify_accounting_failure(
    harness: &native_live_setup::Harness,
    repos: &GatewayRepositories,
    session: &systemprompt_identifiers::SessionId,
) -> Result<Value> {
    let before = summary(harness).await?;
    let Some(record) = before["requests"].as_array().and_then(|rows| {
        rows.iter().find(|row| {
            row["request"]["status"] == "completed" && row["reservation"]["actual"] == 25
        })
    }) else {
        return Ok(
            json!({"verified":false,"reason":"No completed native provider usage to correct"}),
        );
    };
    let request = AiRequestId::new(
        record["request"]["id"]
            .as_str()
            .context("Missing accounted request identity")?,
    );
    let audit = GatewayAudit::new(
        repos,
        GatewayRequestContext {
            ai_request_id: request.clone(),
            user_id: harness.owner.clone(),
            session_id: Some(session.clone()),
            context_id: ContextId::generate(),
            gateway_conversation_id: None,
            client_session_id: None,
            trace_id: Some(TraceId::generate()),
            access_scope: AccessScope::Unknown,
            client_id: None,
            provider: "native-fixture".to_owned(),
            requested_model: Some(harness.native_model.clone()),
            model: harness.native_model.clone(),
            max_tokens: None,
            is_streaming: false,
            origin: RequestOrigin::gateway(
                ClientKind::Other,
                InboundWireProtocol::AnthropicMessages,
            ),
            access_log: None,
        },
    );
    for _ in 0..2 {
        audit
            .accounting_failed("deterministic live native post-response accounting fault")
            .await?;
        ensure!(
            repos
                .evaluations
                .settle_recorded(&harness.owner, &request)
                .await?,
            "Accounted failure lost its evaluation reservation"
        );
    }
    let after = summary(harness).await?;
    ensure!(
        before["budget"] == after["budget"],
        "Accounting failure replay double-settled native spend"
    );
    let retained = after["requests"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row["request"]["id"] == request.as_str())
        })
        .context("Corrected native request disappeared")?;
    ensure!(
        retained["request"]["status"] == "failed"
            && !retained["request"]["accounting_failed_at"].is_null()
            && retained["request"]["input_tokens"] == 11
            && retained["request"]["output_tokens"] == 7
            && retained["request"]["cost_microdollars"] == 25
            && retained["reservation"] == record["reservation"],
        "Native failed-spend projection lost immutable usage or reservation"
    );
    Ok(
        json!({"verified":true,"request_id":request,"journaled_failure_replayed_without_extra_spend":true}),
    )
}
