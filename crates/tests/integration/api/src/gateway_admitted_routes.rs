//! The gateway surface built over an admitted composition and a kernel
//! resolved from it: the admitted gateway route is mounted only by a role
//! that serves the gateway, answers only inside its own paths, alone carries
//! the verifier's `GatewayPrincipal`, and the guards the engines run are the
//! kernel-built guards of the admitted members.

use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::admitted::AdmittedGatewayRoute;
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES};
use systemprompt_extension::router::{
    Allowance, BaseRef, GatewayPath, GatewayPrincipal, GatewayRouteDecl, RouteSet, RouteSpec,
};
use systemprompt_extension::{
    Admitted, FactoryError, FromState, GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard,
    MissingDependency, Resolved,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, gateway_probe_admitted, offline_resolved_kernel, seed_admin_credential,
};
use tower::ServiceExt;

use super::common::setup_ctx_customised;

const WHOAMI: &str = "/v1/bridge/notifications/whoami";
const GATEWAY_RELATIVE_WHOAMI: &str = "/bridge/notifications/whoami";
const OUTSIDE: &str = "/v1/bridge/notifications/elsewhere";
const ANONYMOUS: &str = "anonymous";

#[derive(Clone)]
struct ProbeState;

impl FromState for ProbeState {
    fn from_state(_r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self)
    }
}

async fn whoami(principal: Option<axum::Extension<GatewayPrincipal>>) -> String {
    principal.map_or_else(
        || ANONYMOUS.to_owned(),
        |axum::Extension(principal)| principal.user_id().as_str().to_owned(),
    )
}

fn bridge_route() -> AdmittedGatewayRoute {
    let shape = PRIVILEGED_SHAPES
        .iter()
        .find(|e| {
            e.owner == CoreId::NotificationsHttp
                && e.base == BaseRef::Gateway(GatewayPath::BridgeNotifications)
                && e.allowance == Allowance::GatewayRoute
        })
        .expect("the bridge notifications gateway shape is listed");
    let spec = RouteSpec::new(shape.shape.policy, shape.shape.context, shape.shape.limits);
    AdmittedGatewayRoute {
        owner: ExtensionId::new("notifications-http"),
        decl: GatewayRouteDecl {
            limits: shape.shape.limits,
            router: RouteSet::gateway::<ProbeState>(GatewayPath::BridgeNotifications, shape.group)
                .route(http::Method::GET, GATEWAY_RELATIVE_WHOAMI, whoami, spec)
                .build(),
        },
    }
}

struct DenyAll;

#[async_trait::async_trait]
impl GatewayRequestGuard for DenyAll {
    async fn check(
        &self,
        _db: &dyn systemprompt_traits::DatabaseHandle,
        _request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        Err(GatewayDenyReason::forbidden("probe guard"))
    }
}

fn guard_probe() -> Probe {
    Probe::new("guard-probe", |c| {
        c.gateway_guard(
            |_: &Resolved<'_>| -> Result<Arc<dyn GatewayRequestGuard>, FactoryError> {
                Ok(Arc::new(DenyAll))
            },
        );
    })
}

struct Composition {
    admitted: Admitted,
    kernel: Kernel,
}

fn composition(gateway_route: bool, probes: Vec<Probe>) -> Composition {
    let admitted = gateway_probe_admitted(ComposedParts {
        probes,
        gateway_routes: if gateway_route {
            vec![bridge_route()]
        } else {
            Vec::new()
        },
    });
    let kernel =
        offline_resolved_kernel(&admitted, systemprompt_test_fixtures::gateway_kernel_seed());
    Composition { admitted, kernel }
}

async fn context() -> anyhow::Result<Arc<AppContext>> {
    let (_pool, ctx) = setup_ctx_customised(|config| {
        config.rate_limits = RateLimitsConfig::disabled();
        config.trusted_proxies = Vec::new();
    })
    .await?;
    Ok(ctx)
}

async fn router(composed: &Composition, role: NodeRole) -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx_customised(|config| {
        config.rate_limits = RateLimitsConfig::disabled();
        config.trusted_proxies = Vec::new();
        config.role = role;
    })
    .await?;
    setup_api_server(
        &ctx,
        Composed {
            admitted: &composed.admitted,
            kernel: &composed.kernel,
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
}

async fn get(
    app: &Router,
    uri: &str,
    bearer: Option<&str>,
) -> anyhow::Result<(StatusCode, String)> {
    let mut builder = Request::builder()
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "admitted-gateway-test/1.0");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let mut req = builder.body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

#[tokio::test]
async fn only_a_gateway_role_mounts_the_admitted_gateway_route() -> anyhow::Result<()> {
    let composed = composition(true, Vec::new());

    let gateway = router(&composed, NodeRole::Gateway).await?;
    let api = router(&composed, NodeRole::Api).await?;

    assert_eq!(
        get(&gateway, WHOAMI, None).await?.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(get(&api, WHOAMI, None).await?.0, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn the_admitted_gateway_route_answers_only_inside_its_paths() -> anyhow::Result<()> {
    let app = router(&composition(true, Vec::new()), NodeRole::Gateway).await?;

    assert_ne!(get(&app, WHOAMI, None).await?.0, StatusCode::NOT_FOUND);
    assert_eq!(get(&app, OUTSIDE, None).await?.0, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn without_the_admitted_gateway_route_its_path_is_not_mounted() -> anyhow::Result<()> {
    let app = router(&composition(false, Vec::new()), NodeRole::Gateway).await?;

    assert_eq!(get(&app, WHOAMI, None).await?.0, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn the_admitted_gateway_route_carries_the_verified_principal() -> anyhow::Result<()> {
    let ctx = context().await?;
    let cred = seed_admin_credential(
        ctx.db_pool(),
        &format!("admitted-gw-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let app = router(&composition(true, Vec::new()), NodeRole::Gateway).await?;

    let (status, body) = get(&app, WHOAMI, Some(cred.jwt.as_str())).await?;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, cred.user_id.as_str());
    Ok(())
}

#[tokio::test]
async fn a_route_outside_the_admitted_gateway_routes_carries_no_principal() -> anyhow::Result<()> {
    let ctx = context().await?;
    let cred = seed_admin_credential(
        ctx.db_pool(),
        &format!("plain-gw-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let plain = Router::new().route("/whoami", axum::routing::get(whoami));

    let (status, body) = get(&plain, "/whoami", Some(cred.jwt.as_str())).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, ANONYMOUS);
    Ok(())
}

#[tokio::test]
async fn the_engines_run_the_admitted_guard_probe() -> anyhow::Result<()> {
    let _ctx = context().await?;
    let composed = composition(false, vec![guard_probe()]);

    let engines = composed
        .kernel
        .state()
        .find::<std::sync::Arc<systemprompt_gateway::GatewayEngines>>()
        .cloned()
        .expect("the gateway member provides its engines");

    let owners: Vec<&str> = engines
        .guards
        .installed_owners()
        .into_iter()
        .map(ExtensionId::as_str)
        .collect();
    assert_eq!(owners, vec!["guard-probe"]);
    Ok(())
}

#[tokio::test]
async fn with_no_guard_probe_the_engines_run_no_guard() -> anyhow::Result<()> {
    let _ctx = context().await?;
    let composed = composition(false, Vec::new());

    let engines = composed
        .kernel
        .state()
        .find::<std::sync::Arc<systemprompt_gateway::GatewayEngines>>()
        .cloned()
        .expect("the gateway member provides its engines");

    assert!(engines.guards.is_empty());
    Ok(())
}
