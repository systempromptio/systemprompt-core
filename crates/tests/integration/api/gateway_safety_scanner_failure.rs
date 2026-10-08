//! A safety scanner that fails is never read as clean: an enforcing policy
//! blocks the request and persists a `scanner_failure` finding.

use systemprompt_database::DbPool;
use systemprompt_gateway::protocol::CanonicalContent;
use systemprompt_gateway::protocol::canonical::{CanonicalRequest, CanonicalResponse};
use systemprompt_gateway::service::{DispatchError, GatewayError, GatewayService};
use systemprompt_gateway::{
    CATEGORY_SCANNER_FAILURE, Finding, GatewayRepositories, SafetyScanner, ScanError,
    ScannerSettings, register_safety_scanner,
};
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::providers::ApiSurface;
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_wire::WireProtocol;
use uuid::Uuid;

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

const SCANNER: &str = "test_failing_on_marker";
const MARKER: &str = "scanner-failure-marker";

#[derive(Default)]
struct FailingOnMarkerScanner;

#[async_trait::async_trait]
impl SafetyScanner for FailingOnMarkerScanner {
    fn name(&self) -> &'static str {
        SCANNER
    }

    async fn scan_request(&self, req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        let carries_marker = req
            .safety_parts(false)
            .into_iter()
            .any(|(_, text)| text.contains(MARKER));
        if carries_marker {
            return Err(ScanError::Failed {
                scanner: SCANNER,
                reason: "backend unreachable".to_owned(),
            });
        }
        Ok(Vec::new())
    }

    async fn scan_response_final(
        &self,
        _response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
}

register_safety_scanner!(|_: &ScannerSettings| FailingOnMarkerScanner, name = SCANNER);

async fn install_policy(pool: &DbPool, name: &str) -> anyhow::Result<()> {
    let pg = pool.pool();
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) VALUES ($1, $2, $3, \
         TRUE, 100)",
    )
    .bind(format!("gwpol_{}", Uuid::new_v4().simple()))
    .bind(name)
    .bind(serde_json::json!({ "safety": { "scanners": [SCANNER] } }))
    .execute(pg.as_ref())
    .await?;
    Ok(())
}

async fn settled_categories(
    repos: &GatewayRepositories,
    pool: &DbPool,
    id: &AiRequestId,
) -> Vec<String> {
    assert_eq!(
        repos
            .background
            .drain(std::time::Duration::from_secs(30))
            .await,
        systemprompt_traits::DrainOutcome::Drained
    );
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT category FROM ai_safety_findings WHERE ai_request_id = $1")
            .bind(id.as_str())
            .fetch_all(pool.pool().as_ref())
            .await
            .expect("query findings");
    rows.into_iter().map(|(c,)| c).collect()
}

#[tokio::test]
async fn a_failing_scanner_blocks_the_request_under_an_enforcing_policy() -> anyhow::Result<()> {
    install_provider_api_key();
    let _ = setup_ctx().await?;
    let database = systemprompt_test_fixtures::DisposableDb::with_schema("gw_scan_fail").await;
    let pool = database.test_pool().await;
    let cred = seed_admin_credential(
        &pool,
        &format!("gw-scan-fail-{}@example.invalid", Uuid::new_v4().simple()),
    )
    .await?;
    let policy_name = format!("gw-scan-fail-{}", Uuid::new_v4().simple());
    install_policy(&pool, &policy_name).await?;

    let config = gateway_config(PROVIDER);
    let registry = provider_registry(
        "http://127.0.0.1:1",
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut request = canonical_request(MODEL, false);
    request.messages[0].content = vec![CanonicalContent::text(format!(
        "an ordinary question carrying the {MARKER}"
    ))];
    let di = inputs(&cred, request, false);
    let request_id = di.ctx.ai_request_id.clone();

    let repos = gw_repos(&pool);
    let outcome = GatewayService::dispatch(&config, &registry, &pool, &repos, di).await;
    let categories = settled_categories(&repos, &pool, &request_id).await;
    database.drop_now().await;

    match outcome.expect_err("a failed scan must not be treated as clean") {
        DispatchError::Recorded(inner) => {
            let GatewayError::Safety(blocked) = &inner else {
                panic!("expected SafetyBlocked, got {inner:?}");
            };
            assert_eq!(blocked.category, CATEGORY_SCANNER_FAILURE);
        },
        other => panic!("expected Recorded(SafetyBlocked), got {other:?}"),
    }
    assert!(
        categories.iter().any(|c| c == CATEGORY_SCANNER_FAILURE),
        "the scanner failure is persisted as a finding; got {categories:?}"
    );
    Ok(())
}
