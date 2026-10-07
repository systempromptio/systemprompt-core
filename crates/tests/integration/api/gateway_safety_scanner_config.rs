//! The safety-scanner seam: an extension scanner registered through
//! `register_safety_scanner!` receives its policy's
//! `scanner_settings.<name>.config` mapping uninterpreted by core, and its
//! `fail_mode` decides whether a failed scan blocks the request.

use systemprompt_database::DbPool;
use systemprompt_gateway::protocol::CanonicalContent;
use systemprompt_gateway::protocol::canonical::{CanonicalRequest, CanonicalResponse};
use systemprompt_gateway::service::{DispatchError, GatewayError, GatewayService};
use systemprompt_gateway::{
    CATEGORY_SCANNER_FAILURE, Finding, GatewayRepositories, PHASE_REQUEST, SafetyScanner,
    ScanError, ScannerSettings, Severity, register_safety_scanner,
};
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::providers::ApiSurface;
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_wire::WireProtocol;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

const READER: &str = "test_config_reader";
const UNREACHABLE: &str = "test_config_unreachable";

struct ConfigReadingScanner {
    needle: Option<String>,
    category: String,
}

impl ConfigReadingScanner {
    fn from_settings(settings: &ScannerSettings) -> Self {
        let text = |key: &str| {
            settings
                .config
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Self {
            needle: text("needle"),
            category: text("category").unwrap_or_else(|| "unconfigured".to_owned()),
        }
    }
}

#[async_trait::async_trait]
impl SafetyScanner for ConfigReadingScanner {
    fn name(&self) -> &'static str {
        READER
    }

    async fn scan_request(&self, req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        let Some(needle) = &self.needle else {
            return Err(ScanError::Failed {
                scanner: READER,
                reason: "config.needle was not delivered".to_owned(),
            });
        };
        let hit = req
            .safety_parts(false)
            .into_iter()
            .any(|(_, text)| text.contains(needle.as_str()));
        Ok(hit
            .then(|| Finding {
                phase: PHASE_REQUEST,
                severity: Severity::High,
                category: self.category.clone(),
                excerpt: None,
                scanner: READER,
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

register_safety_scanner!(ConfigReadingScanner::from_settings, name = READER);

struct UnreachableScanner;

#[async_trait::async_trait]
impl SafetyScanner for UnreachableScanner {
    fn name(&self) -> &'static str {
        UNREACHABLE
    }

    async fn scan_request(&self, _req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        Err(ScanError::Failed {
            scanner: UNREACHABLE,
            reason: "vendor endpoint unreachable".to_owned(),
        })
    }

    async fn scan_response_final(
        &self,
        _response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
}

register_safety_scanner!(|_: &ScannerSettings| UnreachableScanner, name = UNREACHABLE);

async fn install_policy(pool: &DbPool, safety: serde_json::Value) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) VALUES ($1, $2, $3, \
         TRUE, 100)",
    )
    .bind(format!("gwpol_{}", Uuid::new_v4().simple()))
    .bind(format!("gw-scan-config-{}", Uuid::new_v4().simple()))
    .bind(serde_json::json!({ "safety": safety }))
    .execute(pool.pool().as_ref())
    .await?;
    Ok(())
}

async fn settled_findings(
    repos: &GatewayRepositories,
    pool: &DbPool,
    id: &AiRequestId,
) -> Vec<(String, bool)> {
    assert_eq!(
        repos
            .background
            .drain(std::time::Duration::from_secs(30))
            .await,
        systemprompt_traits::DrainOutcome::Drained
    );
    sqlx::query_as("SELECT category, blocked FROM ai_safety_findings WHERE ai_request_id = $1")
        .bind(id.as_str())
        .fetch_all(pool.pool().as_ref())
        .await
        .expect("query findings")
}

fn request_with(text: &str) -> CanonicalRequest {
    let mut request = canonical_request(MODEL, false);
    request.messages[0].content = vec![CanonicalContent::text(text)];
    request
}

#[tokio::test]
async fn an_extension_scanner_reads_its_own_config_from_the_policy() -> anyhow::Result<()> {
    install_provider_api_key();
    let _ = setup_ctx().await?;
    let database = systemprompt_test_fixtures::DisposableDb::with_schema("gw_scan_cfg").await;
    let pool = database.test_pool().await;
    let cred = seed_admin_credential(
        &pool,
        &format!("gw-scan-cfg-{}@example.invalid", Uuid::new_v4().simple()),
    )
    .await?;
    install_policy(
        &pool,
        serde_json::json!({
            "scanners": [READER],
            "block_categories": ["vendor_flagged"],
            "scanner_settings": { READER: {
                "config": { "needle": "zebra-needle", "category": "vendor_flagged" }
            }}
        }),
    )
    .await?;
    let registry = provider_registry(
        "http://127.0.0.1:1",
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let di = inputs(&cred, request_with("please look at zebra-needle"), false);
    let id = di.ctx.ai_request_id.clone();
    let repos = gw_repos(&pool);
    let outcome =
        GatewayService::dispatch(&gateway_config(PROVIDER), &registry, &pool, &repos, di).await;
    let findings = settled_findings(&repos, &pool, &id).await;
    database.drop_now().await;

    match outcome.expect_err("the configured needle must block") {
        DispatchError::Recorded(GatewayError::Safety(blocked)) => {
            assert_eq!(blocked.category, "vendor_flagged");
        },
        other => panic!("expected a safety block, got {other:?}"),
    }
    assert_eq!(findings, vec![("vendor_flagged".to_owned(), true)]);
    Ok(())
}

#[tokio::test]
async fn an_open_scanner_failure_is_recorded_and_the_request_proceeds() -> anyhow::Result<()> {
    install_provider_api_key();
    let _ = setup_ctx().await?;
    let database = systemprompt_test_fixtures::DisposableDb::with_schema("gw_scan_open").await;
    let pool = database.test_pool().await;
    let cred = seed_admin_credential(
        &pool,
        &format!("gw-scan-open-{}@example.invalid", Uuid::new_v4().simple()),
    )
    .await?;
    install_policy(
        &pool,
        serde_json::json!({
            "scanners": [UNREACHABLE],
            "scanner_settings": { UNREACHABLE: { "fail_mode": "open", "timeout_ms": 2000 } }
        }),
    )
    .await?;
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_open", "type": "message", "role": "assistant", "model": MODEL,
            "content": [{"type": "text", "text": "ok"}], "stop_reason": "end_turn",
            "usage": {"input_tokens": 3, "output_tokens": 2}
        })))
        .expect(1)
        .mount(&upstream)
        .await;
    let registry = provider_registry(
        &upstream.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let di = inputs(&cred, request_with("an ordinary question"), false);
    let id = di.ctx.ai_request_id.clone();
    let repos = gw_repos(&pool);
    let outcome =
        GatewayService::dispatch(&gateway_config(PROVIDER), &registry, &pool, &repos, di).await;
    let findings = settled_findings(&repos, &pool, &id).await;
    database.drop_now().await;

    let response = outcome.expect("an open scanner failure does not block");
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        findings,
        vec![(CATEGORY_SCANNER_FAILURE.to_owned(), false)],
        "the failure is still persisted, as non-blocking"
    );
    Ok(())
}
