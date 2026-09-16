//! Replays bounded native wire artifacts through production gateway dispatch
//! and accounting.
use anyhow::{Context, Result, ensure};
use axum::body::to_bytes;
use bytes::Bytes;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AiRequestId, ContextId, ModelId, ProviderId, RouteId, SecretName, TraceId,
};
use systemprompt_models::services::{
    ApiSurface, GatewayConfig, GatewayRoute, ModelPricing, ProviderEntry, ProviderModel,
    ProviderRegistry, QuotaFaultMode, WireProtocol,
};
use systemprompt_models::wire::origin::{
    ClientAttestation, ClientEvidence, ClientKind, RequestOrigin,
};
use systemprompt_security::policy::types::AccessScope;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_db_pool, seed_admin_credential,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn read_json(path: &Path) -> Result<Value> {
    ensure!(
        std::fs::metadata(path)?.len() <= 16 * 1024 * 1024,
        "Artifact exceeds16MiB"
    );
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
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
fn context(
    cred: &systemprompt_test_fixtures::AuthedFixture,
    model: &str,
    stream: bool,
    inbound: &dyn InboundAdapter,
) -> GatewayRequestContext {
    GatewayRequestContext {
        ai_request_id: AiRequestId::generate(),
        user_id: cred.user_id.clone(),
        session_id: Some(cred.session_id.clone()),
        context_id: ContextId::generate(),
        gateway_conversation_id: None,
        client_session_id: None,
        trace_id: Some(TraceId::generate()),
        access_scope: AccessScope::Unknown,
        client_id: None,
        provider: "native-fixture".to_owned(),
        requested_model: Some(model.to_owned()),
        model: model.to_owned(),
        max_tokens: Some(512),
        is_streaming: stream,
        origin: RequestOrigin::gateway(ClientKind::Other, inbound.wire(), ClientAttestation::None),
        evidence: ClientEvidence::none(),
        access_log: None,
    }
}
async fn settled(db: &DbPool, id: &AiRequestId, expected: Option<&str>) -> Result<Value> {
    let pg = db.pool_arc()?;
    for _ in 0..100 {
        let row: Option<Value> =
            sqlx::query_scalar("SELECT to_jsonb(r) FROM ai_requests r WHERE id=$1")
                .bind(id.as_str())
                .fetch_optional(pg.as_ref())
                .await?;
        if let Some(row) = row
            && expected.map_or_else(
                || {
                    matches!(
                        row["status"].as_str(),
                        Some("success" | "completed" | "failed" | "rejected")
                    )
                },
                |status| row["status"] == status,
            )
        {
            return Ok(row);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("Gateway accounting did not settle within5seconds")
}
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 2,
        "usage: native_gateway_replay artifact-directory report-file"
    );
    std::fs::write(
        &args[1],
        serde_json::to_vec_pretty(
            &json!({"status":"running","paid_inference":false,"automated_target_enabled":false}),
        )?,
    )?;
    let result = tokio::time::timeout(Duration::from_secs(600), replay())
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Native gateway replay exceeded600seconds")));
    if let Err(error) = &result {
        let mut report = read_json(Path::new(&args[1]))?;
        report["status"] = json!("failed");
        report["error"] = json!(format!("{error:#}"));
        std::fs::write(&args[1], serde_json::to_vec_pretty(&report)?)?;
    }
    result
}
async fn replay() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 2,
        "usage: native_gateway_replay artifact-directory report-file"
    );
    let root = PathBuf::from(&args[0]);
    ensure!(
        root.is_absolute() && root.is_dir(),
        "Artifact directory must be absolute"
    );
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await?;
    let _context = fixture_app_context(&db, &bootstrap.database_url)?;
    let journal = systemprompt_api::services::gateway::audit::journal::GatewayJournal::open(
        systemprompt_config::ProfileBootstrap::get_path()?,
        systemprompt_config::SecretsBootstrap::get()?,
    )?;
    let repos = GatewayRepositories::new(
        &db,
        journal,
        Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(&db)?,
        )),
    )?;
    let cred = seed_admin_credential(
        &db,
        &format!("native-replay-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let mut cases = Vec::new();
    let mut complete_responses = 0usize;
    let mut incomplete_cases = 0usize;
    let mut directories = std::fs::read_dir(&root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    directories.sort();
    ensure!(directories.len() <= 64, "Too many artifact entries");
    for directory in directories
        .into_iter()
        .filter(|directory| directory.is_dir())
    {
        let plan = read_json(&directory.join("plan.json"))?;
        let normalized = read_json(&directory.join("normalized.json"))?;
        let audit = directory.join("evidence/provider.jsonl");
        ensure!(
            std::fs::metadata(&audit)?.len() <= 64 * 1024 * 1024,
            "Native journal exceeds64MiB"
        );
        let events = std::fs::read_to_string(audit)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<serde_json::Result<Vec<_>>>()?;
        ensure!(events.len() <= 1024, "Too many journal events");
        let requests = events
            .iter()
            .filter(|event| event["kind"] == "provider")
            .collect::<Vec<_>>();
        ensure!(requests.len() <= 16, "Too many provider attempts");
        let mut records = Vec::new();
        let mut counted = 0i64;
        let mut case_complete = 0usize;
        let mut case_incomplete = requests.is_empty();
        for retained in requests {
            let response = events.iter().find(|event| {
                event["kind"] == "response" && event["ordinal"] == retained["ordinal"]
            });
            let Some(response) = response else {
                case_incomplete = true;
                records.push(json!({"ordinal":retained["ordinal"],"status":"incomplete_response","accounting_verified":false}));
                continue;
            };
            let (outbound, surface, inbound, upstream_path) = match wire(
                retained["path"].as_str().context("Missing route")?,
            ) {
                Ok(wire) => wire,
                Err(error) => {
                    case_incomplete = true;
                    records.push(json!({"ordinal":retained["ordinal"],"status":"unsupported_route","error":error.to_string(),"accounting_verified":false}));
                    continue;
                },
            };
            let raw = Bytes::from(serde_json::to_vec(&retained["body"])?);
            let request = inbound
                .parse_request(&raw)
                .map_err(|error| anyhow::anyhow!("Native inbound parse: {error}"))?;
            let upstream = MockServer::start().await;
            let status = response["status"]
                .as_u64()
                .context("Missing response status")?;
            ensure!((100..=599).contains(&status), "Invalid response status");
            Mock::given(method("POST"))
                .and(path(upstream_path))
                .respond_with(
                    ResponseTemplate::new(status as u16).set_body_raw(
                        response["body"].as_str().context("Missing response wire")?,
                        response["content_type"]
                            .as_str()
                            .unwrap_or("application/json"),
                    ),
                )
                .mount(&upstream)
                .await;
            let configured = config();
            let unknown_registry = registry(
                &upstream.uri(),
                request.model.as_str(),
                outbound,
                surface,
                false,
            );
            ensure!(
                systemprompt_api::services::gateway::pricing::resolve(
                    "native-fixture",
                    &[request.model.as_str()],
                    Some(&configured),
                    &unknown_registry
                )
                .is_err(),
                "Unknown pricing was invented as measured zero"
            );
            let unknown_ctx = context(
                &cred,
                request.model.as_str(),
                request.stream,
                inbound.as_ref(),
            );
            let unknown_id = unknown_ctx.ai_request_id.clone();
            let unknown_dispatch = tokio::time::timeout(
                Duration::from_secs(15),
                GatewayService::dispatch(
                    &configured,
                    &unknown_registry,
                    &db,
                    &repos,
                    DispatchInputs {
                        request: request.clone(),
                        raw_body: raw.clone(),
                        ctx: unknown_ctx,
                        inbound: Arc::clone(&inbound),
                        forward_headers: vec![],
                        identity_headers: vec![],
                        governance: systemprompt_test_fixtures::default_governance_engine(),
                    },
                ),
            )
            .await
            .context("Unpriced dispatch timed out")?;
            ensure!(
                unknown_dispatch.is_err(),
                "Gateway accepted unpriced native request"
            );
            ensure!(
                upstream
                    .received_requests()
                    .await
                    .unwrap_or_default()
                    .is_empty(),
                "Unpriced native request reached upstream"
            );
            let pg = db.pool_arc()?;
            let unknown_row: Option<Value> =
                sqlx::query_scalar("SELECT to_jsonb(r) FROM ai_requests r WHERE id=$1")
                    .bind(unknown_id.as_str())
                    .fetch_optional(pg.as_ref())
                    .await?;
            ensure!(
                unknown_row.is_none(),
                "Unpriced preflight must not create a measured-zero completion"
            );
            let providers = registry(
                &upstream.uri(),
                request.model.as_str(),
                outbound,
                surface,
                true,
            );
            let ctx = context(
                &cred,
                request.model.as_str(),
                request.stream,
                inbound.as_ref(),
            );
            let id = ctx.ai_request_id.clone();
            let fault_ctx = ctx.clone();
            let dispatch = tokio::time::timeout(
                Duration::from_secs(15),
                GatewayService::dispatch(
                    &configured,
                    &providers,
                    &db,
                    &repos,
                    DispatchInputs {
                        request,
                        raw_body: raw,
                        ctx,
                        inbound,
                        forward_headers: vec![],
                        identity_headers: vec![],
                        governance: systemprompt_test_fixtures::default_governance_engine(),
                    },
                ),
            )
            .await
            .context("Native dispatch timed out")?;
            let dispatch_error = match dispatch {
                Ok(response) => {
                    let _body = tokio::time::timeout(
                        Duration::from_secs(10),
                        to_bytes(response.into_body(), 16 * 1024 * 1024),
                    )
                    .await
                    .context("Native replay body timed out")??;
                    None
                },
                Err(error) => Some(format!("{error:?}")),
            };
            std::fs::write(
                &args[1],
                serde_json::to_vec_pretty(
                    &json!({"status":"settling_completion","paid_inference":false,"automated_target_enabled":false,"request_id":id.as_str(),"dispatch_error":dispatch_error,"artifact":directory}),
                )?,
            )?;
            let row = settled(&db, &id, None).await?;
            let mut failed_accounting_row = None;
            if status == 200 {
                ensure!(
                    row["input_tokens"] == 11 && row["output_tokens"] == 7,
                    "Persisted usage differs from deterministic native wire: {row}"
                );
                ensure!(
                    row["cost_microdollars"] == 25,
                    "Persisted native spend differs from configured1/2rates: {row}"
                );
                counted += 18;
                let audit = GatewayAudit::new(&repos, fault_ctx);
                systemprompt_api::services::gateway::service::finalize::record_accounting_outcome(
                    &audit,
                    QuotaFaultMode::Closed,
                    systemprompt_api::services::gateway::quota::AccountingOutcome::Faulted {
                        message: "deterministic native replay accounting failure".to_owned(),
                    },
                )
                .await;
                std::fs::write(
                    &args[1],
                    serde_json::to_vec_pretty(
                        &json!({"status":"settling_accounting_failure","paid_inference":false,"automated_target_enabled":false,"request_id":id.as_str(),"dispatch_error":dispatch_error,"persisted_completion":row,"artifact":directory}),
                    )?,
                )?;
                let failed = settled(&db, &id, Some("failed")).await?;
                ensure!(
                    failed["status"] == "failed"
                        && failed["cost_microdollars"] == 25
                        && failed["input_tokens"] == 11
                        && failed["output_tokens"] == 7,
                    "Post-response failed spend was not retained: {failed}"
                );
                failed_accounting_row = Some(failed);
            } else {
                ensure!(
                    row["status"] == "failed",
                    "Provider failure did not persist failed status"
                );
            }
            complete_responses += 1;
            case_complete += 1;
            ensure!(
                complete_responses <= 256,
                "Too many replayed native responses"
            );
            records.push(json!({"ordinal":retained["ordinal"],"request_id":id.as_str(),"persisted":row,"post_accounting_fault":failed_accounting_row,"dispatch_error":dispatch_error,"failed_spend_retention_verified":status==200}));
        }
        let advisory = normalized["reported_input_tokens"]
            .as_i64()
            .zip(normalized["reported_output_tokens"].as_i64())
            .map(|(input, output)| input + output);
        if normalized["completion"] == "completed"
            && let Some(tokens) = advisory
        {
            ensure!(
                tokens == counted,
                "Native advisory usage differs from persisted provider usage for {}: native{tokens},gateway{counted}",
                directory.display()
            );
        }
        if case_incomplete {
            incomplete_cases += 1;
        }
        cases.push(json!({"status":if case_incomplete{"incomplete"}else{"passed"},"complete_responses":case_complete,"client":plan["client"],"scenario":plan["scenario"],"native_completion":normalized["completion"],"native_advisory_tokens":advisory,"persisted_wire_tokens":counted,"advisory_parity":advisory.map(|tokens|tokens==counted),"unknown_pricing_not_invented_as_zero":case_complete>0,"requests":records}));
    }
    ensure!(!cases.is_empty(), "No retained native cases found");
    std::fs::write(
        &args[1],
        serde_json::to_vec_pretty(
            &json!({"paid_inference":false,"automated_target_enabled":false,"live_native_gateway_budget_admission_verified":false,"status":if complete_responses>0&&incomplete_cases==0{"passed"}else{"incomplete"},"complete_responses":complete_responses,"incomplete_cases":incomplete_cases,"cases":cases}),
        )?,
    )?;
    ensure!(
        complete_responses > 0,
        "No complete native provider response was replayed"
    );
    ensure!(
        incomplete_cases == 0,
        "Some native cases have incomplete or unsupported wire evidence"
    );
    Ok(())
}
