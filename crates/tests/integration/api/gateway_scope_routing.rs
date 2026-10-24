//! Per-scope upstream routing: a policy route whose `when: scope` matches the
//! value the registered `project` dimension attributes the request to pins its
//! deployment; any other value, or an unattributed request, is served by the
//! model's own chain; and a quota window keyed by the dimension counts the
//! scoped request.

use axum::body::to_bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use systemprompt_api::routes::gateway::messages::extract::RejectionPartial;
use systemprompt_api::routes::gateway::messages::extract::scope::{
    ScopeHeaders, ScopeResolution, resolve_scope_attribution,
};
use systemprompt_catalog_fixtures::{CatalogFixture, platform};
use systemprompt_gateway::GatewayRepositories;
use systemprompt_gateway::service::{DispatchError, GatewayError, GatewayService};
use systemprompt_identifiers::{ApiKeyId, ScopeDimension, UserId};
use systemprompt_manifest::services::catalog::{
    EndpointConfig, EndpointRef, ModelEntry, ProviderConfig, ResolvedCatalog,
};
use systemprompt_manifest::services::{
    GatewayConfig, GatewayRoute, ModelGlobs, RouteDeployment, RouteMatch,
};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_models::origin::{
    ClientAttestation, ClientKind, InboundWireProtocol, RequestOrigin,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::{AuthedFixture, DisposableDb, seed_admin_credential};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
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

const SCOPED_ROUTE: &str = "scope-p-other";

struct Topology {
    shared: MockServer,
    scoped: MockServer,
    registry: ResolvedCatalog,
    scoped_name: String,
}

// Why: `MODEL` is deployed on the shared endpoint first and the scoped one
// second, so a request no policy route narrows is served by the shared
// upstream; the `p-other` project route pins the scoped deployment.
async fn topology() -> Topology {
    let shared = upstream("shared served").await;
    let scoped = upstream("scoped served").await;
    let shared_name = format!("vertex-shared-{}", uuid::Uuid::new_v4().simple());
    let scoped_name = format!("vertex-scoped-{}", uuid::Uuid::new_v4().simple());
    let scope = [(
        ScopeDimension::try_new("project").expect("dimension"),
        "p-other".to_owned(),
    )]
    .into_iter()
    .collect();
    let registry = CatalogFixture::new()
        .credential(&shared_name, platform("anthropic"))
        .credential(&scoped_name, platform("anthropic"))
        .provider(&shared_name, ProviderConfig::new(&shared_name))
        .provider(&scoped_name, ProviderConfig::new(&scoped_name))
        .endpoint(
            &shared_name,
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, shared.uri()),
        )
        .endpoint(
            &scoped_name,
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, scoped.uri()),
        )
        .model(MODEL, ModelEntry::new(vec![ClientFamily::Claude]))
        .deploy(
            MODEL,
            [
                format!("{shared_name}/api").as_str(),
                format!("{scoped_name}/api").as_str(),
            ],
        )
        .route(GatewayRoute {
            models: ModelGlobs(vec![MODEL.to_owned()]),
            when: Some(RouteMatch {
                scope,
                ..RouteMatch::default()
            }),
            deployments: vec![RouteDeployment::Endpoint(EndpointRef::new(
                &scoped_name,
                "api",
            ))],
            ..GatewayRoute::new(SCOPED_ROUTE)
        })
        .build();
    Topology {
        shared,
        scoped,
        registry,
        scoped_name,
    }
}

impl Topology {
    fn config(&self) -> GatewayConfig {
        gateway_config(&self.scoped_name)
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
            subject: &repos
                .subject_providers
                .gather(&user)
                .await
                .expect("subject attributes"),
            headers: &scope_headers,
            key_bindings: &[],
            api_key_id: Some(&api_key_id),
            require: &[],
        },
        &mut partial,
    )
    .expect("attribution resolves through the registered project provider")
}

async fn send(
    pool: &systemprompt_database::DbPool,
    repos: &GatewayRepositories,
    cred: &AuthedFixture,
    config: &GatewayConfig,
    registry: &ResolvedCatalog,
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
    let config = topo.config();
    let response = GatewayService::dispatch(&config, &topo.registry, &pool, &repos, dispatch)
        .await
        .expect("scoped dispatch succeeds");
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("scoped served"));
    assert_eq!(hits(&topo.scoped).await, 1);
    assert_eq!(hits(&topo.shared).await, 0, "the shared chain is not used");
    let (provider, endpoint, route_match): (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT provider, endpoint, route_match FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool.pool().as_ref())
            .await?;
    assert_eq!(provider.as_deref(), Some(topo.scoped_name.as_str()));
    assert_eq!(endpoint, Some(format!("{}/api", topo.scoped_name)));
    assert!(
        route_match
            .unwrap_or_default()
            .contains(&format!("route:{SCOPED_ROUTE}"))
    );
    Ok(())
}

#[tokio::test]
async fn other_and_unattributed_requests_take_the_models_chain() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "scope-route-shared@example.invalid").await?;
    let repos = gw_repos(&pool);
    let topo = topology().await;
    let config = topo.config();
    let other = attribute(&repos, Some("p-primary")).await;
    let body = send(&pool, &repos, &cred, &config, &topo.registry, other).await?;
    assert!(body.contains("shared served"), "{body}");
    let body = send(
        &pool,
        &repos,
        &cred,
        &config,
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
    let config = topo.config();
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
