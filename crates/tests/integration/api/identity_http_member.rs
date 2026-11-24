//! The `identity-http` member's surface as the full server mounts it: with
//! the member admitted its OAuth, discovery, session and API-key routes are
//! served (an anonymous caller is refused where a session is required) and the
//! API-key routes share the admin router's one per-caller throttle; with it
//! disabled none of them is mounted (404). Admitted with no identity surface,
//! or with no SSO slot built, boot is refused naming the cause.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request};
use systemprompt_api::services::server::setup_api_server;
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, fixture_admitted, fixture_admitted_of, fixture_admitted_without,
    fixture_app_context_with_extensions, fixture_config, fixture_core_members,
    seed_admin_credential, test_db_pool,
};
use tower::ServiceExt;

const PUBLIC: [(&str, &str); 3] = [
    ("GET", "/.well-known/oauth-authorization-server"),
    ("GET", "/api/v1/core/oauth/health"),
    ("GET", "/auth/link-passkey"),
];
const GATED: [(&str, &str); 4] = [
    ("GET", "/api/v1/core/oauth/userinfo"),
    ("POST", "/api/v1/core/oauth/revoke"),
    ("POST", "/api/v1/core/users/me/sessions/revoke_all"),
    ("GET", "/api/v1/admin/api-keys"),
];

async fn server(admitted: Arc<systemprompt_extension::Admitted>) -> Router {
    built(admitted, None).await.expect("server builds")
}

async fn built(
    admitted: Arc<systemprompt_extension::Admitted>,
    rate_limits: Option<RateLimitsConfig>,
) -> anyhow::Result<Router> {
    let boot = ensure_test_bootstrap();
    let mut config = fixture_config(&boot.database_url);
    if let Some(rate_limits) = rate_limits {
        config.rate_limits = rate_limits;
        config.trusted_proxies = Vec::new();
    }
    let ctx = fixture_app_context_with_extensions(config, admitted).await?;
    let router = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    ?;
    Ok(router.layer(axum::Extension(Arc::new(
        systemprompt_api::services::server::SchedulerHealth::new(),
    ))))
}

async fn status(app: &Router, method: &str, uri: &str) -> u16 {
    authed(app, method, uri, None).await
}

async fn authed(app: &Router, method: &str, uri: &str, bearer: Option<&JwtToken>) -> u16 {
    let mut req = Request::builder()
        .method(Method::from_bytes(method.as_bytes()).expect("method"))
        .uri(uri)
        .header("host", "127.0.0.1")
        .header("user-agent", "identity-member-test/1.0");
    if let Some(token) = bearer {
        req = req.header("authorization", format!("Bearer {}", token.as_str()));
    }
    let mut req = req.body(Body::empty()).expect("request");
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.7:40000"
            .parse::<std::net::SocketAddr>()
            .expect("addr"),
    ));
    app.clone()
        .oneshot(req)
        .await
        .expect("response")
        .status()
        .as_u16()
}

#[tokio::test]
async fn the_admitted_member_serves_its_routes_behind_their_gates() {
    let app = server(fixture_admitted()).await;
    for (method, uri) in PUBLIC {
        assert_eq!(status(&app, method, uri).await, 200, "{uri}");
    }
    for (method, uri) in GATED {
        assert_eq!(status(&app, method, uri).await, 401, "{uri}");
    }
}

#[tokio::test]
async fn a_disabled_identity_http_member_mounts_none_of_its_routes() {
    let app = server(fixture_admitted_without(&[ExtensionId::new(
        "identity-http",
    )]))
    .await;
    for (method, uri) in PUBLIC.into_iter().chain(GATED) {
        assert_eq!(status(&app, method, uri).await, 404, "{uri} is not mounted");
    }
    let admin = seed_admin_credential(&test_db_pool().await, "identity-disabled-admin")
        .await
        .expect("admin credential");
    assert_eq!(
        authed(&app, "GET", API_KEYS, Some(&admin.jwt)).await,
        404,
        "an admin meets no API-key route either"
    );
}

const API_KEYS: &str = "/api/v1/admin/api-keys";
const ADMIN_STATUS: &str = "/api/v1/admin/services/status";

#[tokio::test]
async fn the_api_key_routes_share_the_admin_routers_throttle() {
    let app = built(
        fixture_admitted(),
        Some(RateLimitsConfig {
            burst_multiplier: 1,
            disabled: false,
            ..RateLimitsConfig::testing()
        }),
    )
    .await
    .expect("server builds");
    let admin = seed_admin_credential(&test_db_pool().await, "identity-admin-throttle")
        .await
        .expect("admin credential");
    let mut drained = Vec::new();
    for _ in 0..30 {
        drained.push(authed(&app, "GET", ADMIN_STATUS, Some(&admin.jwt)).await);
    }
    assert!(
        drained.contains(&429),
        "the admin budget is spent: {drained:?}"
    );
    let mut keys = Vec::new();
    for _ in 0..3 {
        keys.push(authed(&app, "GET", API_KEYS, Some(&admin.jwt)).await);
    }
    assert!(
        keys.contains(&429),
        "the API-key routes draw on the same admin budget: {keys:?}"
    );
}

async fn refusal(admitted: Arc<systemprompt_extension::Admitted>) -> String {
    let err = built(admitted, None)
        .await
        .err()
        .expect("boot is refused");
    format!("{err:#}")
}

#[tokio::test]
async fn an_admitted_identity_http_without_its_surface_refuses_boot() {
    let members = fixture_core_members()
        .into_iter()
        .map(|member| -> Box<dyn systemprompt_extension::Extension> {
            if member.metadata().id == "identity-http" {
                Box::new(Probe::new("identity-http", |_| {}))
            } else {
                member
            }
        })
        .collect();
    let chain = refusal(fixture_admitted_of(members)).await;
    assert!(
        chain.contains("identity-http is admitted but provides no identity surface"),
        "{chain}"
    );
}

#[tokio::test]
async fn an_admitted_identity_http_without_the_sso_slot_refuses_boot() {
    let chain = refusal(fixture_admitted_without(&[ExtensionId::new("identity")])).await;
    assert!(
        chain.contains("identity-http is admitted but the SSO slot was never built"),
        "{chain}"
    );
}
