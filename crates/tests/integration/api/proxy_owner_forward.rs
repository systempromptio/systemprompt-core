//! An `api` replica supervises no processes, so the MCP proxy forwards each
//! request to the replica that runs the process: the owner resolved from the
//! services, endpoint and presence rows, stamped with an owner-hop marker
//! signed for that service. With no live owner the proxy answers 503
//! `no_live_owner`. A worker ignores a marker a client supplies and forwards
//! the request to the owner as if it had arrived directly. For a process the
//! configuration supervises, the `api` replica holds the request while the
//! process is between owners and forwards it once the new owner registers;
//! when none appears within the wait it answers 503, and at once thereafter.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use systemprompt_api::services::proxy::service::owner::{
    HandoverWaits, OWNER_HANDOVER_POLL, OWNER_HANDOVER_WAIT, OwnerCache, owner_url,
};
use systemprompt_database::{
    CreateServiceInput, ProcessOwner, ServiceModule, ServiceRepository, ServiceStatus,
};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_security::jwt::verify_owner_hop;
use systemprompt_test_fixtures::{
    TestBootstrap, ensure_test_bootstrap, fixture_app_context_with_config,
    fixture_app_context_with_services, fixture_config, init_services_bootstrap, test_db_pool,
};
use tower::ServiceExt;
use wiremock::matchers::{header_exists, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{request_context, with_config};

async fn app_with_role(
    role: NodeRole,
) -> anyhow::Result<(axum::Router, std::sync::Arc<AppContext>)> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&b.database_url);
    config.role = role;
    let ctx = fixture_app_context_with_config(&pool, config)?;
    Ok((
        with_config(systemprompt_api::routes::proxy::mcp::router(&ctx), &ctx),
        ctx,
    ))
}

async fn api_app() -> anyhow::Result<axum::Router> {
    Ok(app_with_role(NodeRole::Api).await?.0)
}

fn mcp_request(service: &str, ctx: RequestContext) -> anyhow::Result<Request<Body>> {
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/{service}/mcp?probe=1"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))?;
    request.extensions_mut().insert(ctx);
    Ok(request)
}

async fn live_owner(owner: &MockServer) -> anyhow::Result<ServiceName> {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let service = ServiceName::new(format!("owned-{}", &suffix[..10]));
    register_owner(&owner.uri(), &service).await?;
    Ok(service)
}

async fn register_owner(advertise_url: &str, service: &ServiceName) -> anyhow::Result<()> {
    let pool = test_db_pool().await;
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let rows = ServiceRepository::new(&pool, InstanceId::new(format!("owner_{suffix}")));
    rows.advertise_endpoint(advertise_url).await?;
    rows.touch_presence(NodeRole::All).await?;
    rows.create_service(CreateServiceInput {
        name: service,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: 5601,
        binary_mtime: None,
    })
    .await?;
    Ok(())
}

struct SupervisedApi {
    app: axum::Router,
    service: ServiceName,
    _boot: TestBootstrap,
}

async fn api_supervising_one_mcp_server() -> anyhow::Result<SupervisedApi> {
    let name = format!("moving_{}", uuid::Uuid::new_v4().simple());
    let services = format!(
        r#"mcp_servers:
  {name}:
    server_type: internal
    binary: fixture
    package: fixture
    port: 5602
    enabled: true
    tool_policy: allow
    display_in_web: false
    oauth:
      required: true
      scopes: [admin]
      audience: mcp
      client_id: null
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
"#
    );
    let boot = init_services_bootstrap(&services);
    let manifest_dir = boot.system_path.join("extensions").join("fixture");
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::write(
        manifest_dir.join("manifest.yaml"),
        "extension:\n  type: mcp\n  name: fixture\n  binary: fixture\n",
    )?;
    let pool = test_db_pool().await;
    let state = boot.services.clone();
    let mut config = fixture_config(&boot.database_url);
    config.role = NodeRole::Api;
    config.system_path = boot.system_path.display().to_string();
    config.services_path = boot.services_path.display().to_string();
    config.bin_path = boot.bin_path.display().to_string();
    let ctx = fixture_app_context_with_services(&pool, config, state, true)?;
    Ok(SupervisedApi {
        app: with_config(systemprompt_api::routes::proxy::mcp::router(&ctx), &ctx),
        service: ServiceName::new(name),
        _boot: boot,
    })
}

fn markers_verify(ctx: &AppContext, service: &ServiceName, received: &[wiremock::Request]) -> bool {
    received.iter().all(|request| {
        request
            .headers
            .get(systemprompt_identifiers::headers::OWNER_HOP)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|marker| {
                verify_owner_hop(
                    ctx.signing_keys().keyring(),
                    &ctx.config().jwt_issuer,
                    service,
                    marker,
                )
                .is_ok()
            })
    })
}

#[tokio::test]
async fn an_api_replica_forwards_to_the_live_owner_with_a_signed_marker() -> anyhow::Result<()> {
    let owner = MockServer::start().await;
    let service = live_owner(&owner).await?;

    Mock::given(method("POST"))
        .and(header_exists(systemprompt_identifiers::headers::OWNER_HOP))
        .respond_with(ResponseTemplate::new(200).set_body_string("from-owner"))
        .expect(2)
        .mount(&owner)
        .await;

    let (app, ctx) = app_with_role(NodeRole::Api).await?;
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(mcp_request(
                service.as_str(),
                request_context("owner-forward"),
            )?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1 << 16).await?;
        assert_eq!(&body[..], b"from-owner");
    }
    let forwarded = owner.received_requests().await.unwrap_or_default();
    assert!(
        forwarded.iter().all(|r| r.url.query() == Some("probe=1")),
        "the query string travels to the owner"
    );
    assert!(
        markers_verify(&ctx, &service, &forwarded),
        "every forwarded request carries a marker signed for this service"
    );
    Ok(())
}

#[tokio::test]
async fn a_worker_ignores_a_client_supplied_owner_hop_marker() -> anyhow::Result<()> {
    let owner = MockServer::start().await;
    let service = live_owner(&owner).await?;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("from-owner"))
        .expect(1)
        .mount(&owner)
        .await;

    let (app, ctx) = app_with_role(NodeRole::Worker).await?;
    let mut request = mcp_request(service.as_str(), request_context("owner-forged"))?;
    request.headers_mut().insert(
        systemprompt_identifiers::headers::OWNER_HOP,
        axum::http::HeaderValue::from_static("1"),
    );
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1 << 16).await?;
    assert_eq!(&body[..], b"from-owner");
    let forwarded = owner.received_requests().await.unwrap_or_default();
    assert!(markers_verify(&ctx, &service, &forwarded));
    Ok(())
}

#[tokio::test]
async fn an_api_replica_with_no_live_owner_answers_service_unavailable() -> anyhow::Result<()> {
    let app = api_app().await?;
    let service = format!(
        "orphan-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..10]
    );
    let response = app
        .oneshot(mcp_request(
            service.as_str(),
            request_context("owner-missing"),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = String::from_utf8(to_bytes(response.into_body(), 1 << 16).await?.to_vec())?;
    assert!(
        body.contains("no_live_owner") || body.contains(&service),
        "{body}"
    );
    Ok(())
}

#[test]
fn owner_urls_join_cleanly_and_the_cache_returns_what_was_put() {
    assert_eq!(owner_url("http://a:1/", "/x?y=1"), "http://a:1/x?y=1");
    assert_eq!(owner_url("http://a:1", "x"), "http://a:1/x");
    let cache = OwnerCache::default();
    let name = ServiceName::new("cached");
    assert!(cache.get(&name).is_none());
    let owner = ProcessOwner {
        instance_id: systemprompt_test_fixtures::unique_instance(),
        advertise_url: "http://a:1".to_owned(),
    };
    cache.put(&name, &owner);
    assert_eq!(
        cache.get(&name).map(|o| o.advertise_url),
        Some("http://a:1".to_owned())
    );
}

#[tokio::test]
async fn an_api_replica_holds_a_request_until_the_new_owner_registers() -> anyhow::Result<()> {
    let owner = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("from-new-owner"))
        .expect(1)
        .mount(&owner)
        .await;
    let api = api_supervising_one_mcp_server().await?;

    let gap = std::time::Duration::from_millis(700);
    let (uri, service) = (owner.uri(), api.service.clone());
    let registering = tokio::spawn(async move {
        tokio::time::sleep(gap).await;
        register_owner(&uri, &service).await
    });

    let started = std::time::Instant::now();
    let response = api
        .app
        .clone()
        .oneshot(mcp_request(
            api.service.as_str(),
            request_context("owner-moving"),
        )?)
        .await?;
    let waited = started.elapsed();
    registering.await??;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1 << 16).await?;
    assert_eq!(&body[..], b"from-new-owner");
    assert!(
        waited >= gap,
        "the request was held through the gap: {waited:?}"
    );
    assert!(
        waited < OWNER_HANDOVER_WAIT,
        "the request was forwarded when the owner registered, not at the deadline: {waited:?}"
    );
    Ok(())
}

#[tokio::test]
async fn a_supervised_process_nobody_claims_is_refused_after_the_wait_and_then_at_once()
-> anyhow::Result<()> {
    let api = api_supervising_one_mcp_server().await?;

    let started = std::time::Instant::now();
    let first = api
        .app
        .clone()
        .oneshot(mcp_request(
            api.service.as_str(),
            request_context("owner-absent"),
        )?)
        .await?;
    let waited = started.elapsed();
    assert_eq!(first.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        waited >= OWNER_HANDOVER_WAIT,
        "the first request waits out the hand-over window: {waited:?}"
    );

    let started = std::time::Instant::now();
    let second = api
        .app
        .clone()
        .oneshot(mcp_request(
            api.service.as_str(),
            request_context("owner-absent"),
        )?)
        .await?;
    let waited = started.elapsed();
    assert_eq!(second.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        waited < OWNER_HANDOVER_WAIT / 2,
        "a request after the window is refused without a second wait: {waited:?}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unsupervised_name_is_refused_without_waiting() -> anyhow::Result<()> {
    let app = api_app().await?;
    let service = format!(
        "orphan-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..10]
    );
    let started = std::time::Instant::now();
    let response = app
        .oneshot(mcp_request(
            service.as_str(),
            request_context("owner-missing"),
        )?)
        .await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        started.elapsed() < OWNER_HANDOVER_WAIT / 2,
        "a name the configuration does not supervise is never waited for: {:?}",
        started.elapsed()
    );
    Ok(())
}

#[test]
fn requests_for_one_service_share_a_deadline_and_ration_registry_reads() {
    let waits = HandoverWaits::default();
    let name = ServiceName::new("moving");
    let start = std::time::Instant::now();

    let deadline = waits.deadline(&name, start);
    assert_eq!(deadline, start + OWNER_HANDOVER_WAIT);
    assert_eq!(
        waits.deadline(&name, start + std::time::Duration::from_secs(3)),
        deadline,
        "a later request joins the wait already running"
    );

    assert!(!waits.probe_due(&name, start + OWNER_HANDOVER_POLL / 2));
    assert!(waits.probe_due(&name, start + OWNER_HANDOVER_POLL));
    assert!(
        !waits.probe_due(&name, start + OWNER_HANDOVER_POLL + OWNER_HANDOVER_POLL / 2),
        "one read per poll interval however many requests wait"
    );

    waits.clear(&name);
    let later = start + OWNER_HANDOVER_WAIT * 2;
    assert_eq!(
        waits.deadline(&name, later),
        later + OWNER_HANDOVER_WAIT,
        "a found owner ends the wait, so the next hand-over starts a new one"
    );
}
