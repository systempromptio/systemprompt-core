//! An `api` replica supervises no processes, so the MCP proxy forwards each
//! request to the replica that runs the process: the owner resolved from the
//! services, endpoint and presence rows, stamped with the owner-hop header.
//! With no live owner the proxy answers 503 `no_live_owner`.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use systemprompt_api::services::proxy::service::owner::{OwnerCache, owner_url};
use systemprompt_database::{
    CreateServiceInput, ProcessOwner, ServiceModule, ServiceRepository, ServiceStatus,
};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
};
use tower::ServiceExt;
use wiremock::matchers::{header, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{request_context, with_config};

async fn api_app() -> anyhow::Result<axum::Router> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&b.database_url);
    config.role = NodeRole::Api;
    let ctx = fixture_app_context_with_config(&pool, config)?;
    Ok(with_config(
        systemprompt_api::routes::proxy::mcp::router(&ctx),
        &ctx,
    ))
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

#[tokio::test]
async fn an_api_replica_forwards_to_the_live_owner_with_the_hop_header() -> anyhow::Result<()> {
    let owner = MockServer::start().await;
    let pool = test_db_pool().await;
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let service = ServiceName::new(format!("owned-{}", &suffix[..10]));
    let instance = InstanceId::new(format!("owner_{suffix}"));
    let rows = ServiceRepository::new(&pool, instance);
    rows.advertise_endpoint(&owner.uri()).await?;
    rows.touch_presence(NodeRole::All).await?;
    rows.create_service(CreateServiceInput {
        name: &service,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: 5601,
        binary_mtime: None,
    })
    .await?;

    Mock::given(method("POST"))
        .and(header(systemprompt_identifiers::headers::OWNER_HOP, "1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("from-owner"))
        .expect(2)
        .mount(&owner)
        .await;

    let app = api_app().await?;
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
        instance_id: InstanceId::new("i1"),
        advertise_url: "http://a:1".to_owned(),
    };
    cache.put(&name, &owner);
    assert_eq!(
        cache.get(&name).map(|o| o.advertise_url),
        Some("http://a:1".to_owned())
    );
}
