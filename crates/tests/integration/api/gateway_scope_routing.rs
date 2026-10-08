//! Per-scope upstream routing: a route's `by_scope` chain is selected by the
//! value the registered `project` dimension attributes the request to, an
//! unmapped value is refused before any upstream call unless the route opts
//! into `unmapped: shared`, and a quota window keyed by the dimension counts
//! the scoped request.

use std::collections::BTreeMap;

use axum::body::to_bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use systemprompt_api::routes::gateway::messages::extract::RejectionPartial;
use systemprompt_api::routes::gateway::messages::extract::scope::{
    ScopeHeaders, ScopeResolution, resolve_scope_attribution,
};
use systemprompt_gateway::GatewayRepositories;
use systemprompt_gateway::service::{DispatchError, GatewayError, GatewayService};
use systemprompt_identifiers::{ApiKeyId, ProviderId, ScopeDimension, UserId};
use systemprompt_manifest::services::{
    GatewayConfig, ProviderRegistry, RouteScopeChains, ScopeChain, UnmappedScope,
};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_models::origin::{
    ClientAttestation, ClientKind, InboundWireProtocol, RequestOrigin,
};
use systemprompt_models::providers::ApiSurface;
use systemprompt_test_fixtures::{AuthedFixture, DisposableDb, seed_admin_credential};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

const SCOPED_USER_PREFIX: &str = "scope-attr-";

fn completion(text: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "msg_scope_route", "type": "message", "role": "assistant", "model": MODEL,
        "content": [{"type": "text", "text": text}], "stop_reason": "end_turn",
        "usage": {"input_tokens": 3, "output_tokens": 2}
    })
}

async fn upstream(text: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(completion(text)))
        .mount(&server)
        .await;
    server
}

async fn hits(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .expect("request history")
        .len()
}

struct Topology {
    shared: MockServer,
    scoped: MockServer,
    registry: ProviderRegistry,
    shared_name: String,
    scoped_name: String,
}

async fn topology() -> Topology {
    let shared = upstream("shared served").await;
    let scoped = upstream("scoped served").await;
    let shared_name = format!("vertex-shared-{}", uuid::Uuid::new_v4().simple());
    let scoped_name = format!("vertex-scoped-{}", uuid::Uuid::new_v4().simple());
    let mut registry = provider_registry(
        &shared.uri(),
        &shared_name,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut entry = registry.providers[0].clone();
    entry.name = ProviderId::new(&scoped_name);
    entry.endpoint = scoped.uri();
    registry.providers.push(entry);
    Topology {
        shared,
        scoped,
        registry,
        shared_name,
        scoped_name,
    }
}

impl Topology {
    fn config(&self, unmapped: UnmappedScope) -> GatewayConfig {
        let mut config = gateway_config(&self.shared_name);
        let mut chains = BTreeMap::new();
        chains.insert(
            "p-other".to_owned(),
            ScopeChain {
                provider: ProviderId::new(&self.scoped_name),
                upstream_model: None,
                fallbacks: Vec::new(),
                strategy: systemprompt_manifest::services::SelectionStrategy::Ordered,
                weight: None,
                context_fallbacks: Vec::new(),
            },
        );
        config.routes[0].by_scope = Some(RouteScopeChains {
            dimension: ScopeDimension::try_new("project").expect("dimension"),
            chains,
            unmapped,
        });
        config
    }
}

async fn attribute(repos: &GatewayRepositories, header: Option<&str>) -> RequestAttribution {
    let mut map = HeaderMap::new();
    if let Some(value) = header {
        map.insert(
            HeaderName::from_static("x-systemprompt-scope-project"),
            HeaderValue::from_str(value).expect("header value"),
        );
    }
    let scope_headers = ScopeHeaders::capture(&map).expect("scope headers");
    let user = UserId::new(format!("{SCOPED_USER_PREFIX}{}", uuid::Uuid::new_v4()));
    let api_key_id = ApiKeyId::new("key-scope-routing");
    let mut partial = RejectionPartial::new(RequestOrigin::gateway(
        ClientKind::Other,
        InboundWireProtocol::AnthropicMessages,
        ClientAttestation::None,
    ));
    resolve_scope_attribution(
        ScopeResolution {
            providers: &repos.subject_providers,
            user_id: &user,
            headers: &scope_headers,
            key_bindings: &[],
            api_key_id: Some(&api_key_id),
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect("attribution resolves through the registered project provider")
}

async fn send(
    pool: &systemprompt_database::DbPool,
    repos: &GatewayRepositories,
    cred: &AuthedFixture,
    config: &GatewayConfig,
    registry: &ProviderRegistry,
    attribution: RequestAttribution,
) -> Result<String, DispatchError> {
    let mut dispatch = inputs(cred, canonical_request(MODEL, false), false);
    dispatch.ctx.attribution = attribution;
    let response = GatewayService::dispatch(config, registry, pool, repos, dispatch).await?;
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn a_mapped_scope_value_reaches_its_own_deployment_and_records_the_scope()
-> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "scope-route-mapped@example.invalid").await?;
    let repos = gw_repos(&pool);
    let topo = topology().await;
    let attribution = attribute(&repos, Some("p-other")).await;
    let mut dispatch = inputs(&cred, canonical_request(MODEL, false), false);
    dispatch.ctx.attribution = attribution;
    let id = dispatch.ctx.ai_request_id.clone();
    let config = topo.config(UnmappedScope::Deny);
    let response = GatewayService::dispatch(&config, &topo.registry, &pool, &repos, dispatch)
        .await
        .expect("scoped dispatch succeeds");
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("scoped served"));
    assert_eq!(hits(&topo.scoped).await, 1);
    assert_eq!(hits(&topo.shared).await, 0, "the shared chain is not used");
    let (served, route_match): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT served_provider, route_match FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool.pool().as_ref())
            .await?;
    assert_eq!(served.as_deref(), Some(topo.scoped_name.as_str()));
    assert!(
        route_match
            .unwrap_or_default()
            .contains("scope:project=p-other")
    );
    Ok(())
}

#[tokio::test]
async fn an_unmapped_scope_value_is_denied_before_any_upstream_call() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "scope-route-unmapped@example.invalid").await?;
    let repos = gw_repos(&pool);
    let topo = topology().await;
    let attribution = attribute(&repos, None).await;
    assert_eq!(attribution.value_for("project"), Some("p-primary"));
    let config = topo.config(UnmappedScope::Deny);
    let error = send(&pool, &repos, &cred, &config, &topo.registry, attribution)
        .await
        .expect_err("an unmapped value is refused");
    let DispatchError::PreAudit(GatewayError::PolicyDenied(denied)) = error else {
        panic!("expected a pre-audit policy denial, got {error:?}");
    };
    assert!(denied.0.contains("project='p-primary'"), "{}", denied.0);
    assert_eq!(hits(&topo.shared).await + hits(&topo.scoped).await, 0);
    Ok(())
}

#[tokio::test]
async fn shared_opt_in_and_unattributed_requests_take_the_route_chain() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "scope-route-shared@example.invalid").await?;
    let repos = gw_repos(&pool);
    let topo = topology().await;
    let shared = topo.config(UnmappedScope::Shared);
    let unmapped = attribute(&repos, Some("p-primary")).await;
    let body = send(&pool, &repos, &cred, &shared, &topo.registry, unmapped).await?;
    assert!(body.contains("shared served"), "{body}");
    let denying = topo.config(UnmappedScope::Deny);
    let body = send(
        &pool,
        &repos,
        &cred,
        &denying,
        &topo.registry,
        RequestAttribution::none(),
    )
    .await?;
    assert!(body.contains("shared served"), "{body}");
    assert_eq!(hits(&topo.shared).await, 2);
    assert_eq!(hits(&topo.scoped).await, 0);
    Ok(())
}

#[tokio::test]
async fn a_quota_window_on_the_routing_dimension_counts_the_scoped_request() -> anyhow::Result<()> {
    install_provider_api_key();
    let _ = setup_ctx().await?;
    let database = DisposableDb::with_schema("scope_route_quota").await;
    let pool = database.test_pool().await;
    let cred = seed_admin_credential(&pool, "scope-route-quota@example.invalid").await?;
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) \
         VALUES ('gwpol_scope_route', 'gwpol_scope_route', $1, TRUE, 100)",
    )
    .bind(serde_json::json!({
        "quota_windows": [{"window_seconds": 3600, "subject": "project", "max_requests": 1}]
    }))
    .execute(pool.pool().as_ref())
    .await?;
    let repos = gw_repos(&pool);
    let topo = topology().await;
    let config = topo.config(UnmappedScope::Deny);
    let first = attribute(&repos, Some("p-other")).await;
    let body = send(&pool, &repos, &cred, &config, &topo.registry, first).await?;
    assert!(body.contains("scoped served"), "{body}");
    let second = attribute(&repos, Some("p-other")).await;
    let denied = send(&pool, &repos, &cred, &config, &topo.registry, second)
        .await
        .expect_err("the project window is spent");
    assert!(
        matches!(
            denied,
            DispatchError::PreAudit(GatewayError::Quota(_))
                | DispatchError::Recorded(GatewayError::Quota(_))
        ),
        "{denied:?}"
    );
    assert_eq!(
        repos
            .background
            .drain(std::time::Duration::from_secs(30))
            .await,
        systemprompt_traits::DrainOutcome::Drained
    );
    let counted: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(requests), 0)::BIGINT FROM ai_quota_buckets \
         WHERE subject_kind = 'project' AND subject_id = 'p-other'",
    )
    .fetch_one(pool.pool().as_ref())
    .await?;
    assert!(
        counted >= 1,
        "the scoped request is counted under its project"
    );
    assert_eq!(hits(&topo.scoped).await, 1);
    drop(repos);
    pool.pool().close().await;
    database.drop_now().await;
    Ok(())
}
