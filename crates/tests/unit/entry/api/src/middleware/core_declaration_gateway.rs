//! Gateway route registrations are core declarations: only a registration a
//! `Gateway`-group row lists by extension id, exact path, scope and budget is
//! admitted, registrations may not overlap, and an extension's routers are
//! built only on a role that could serve them.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use systemprompt_api::routes::gateway::builtin::{builtin_gateway_prefixes, confined};
use systemprompt_api::routes::gateway::registered::{
    GatewayCredential, GatewayRouteRegistration, admit_registrations,
};
use systemprompt_api::services::middleware::{
    CoreDeclaration, CoreShapeField, MountError, extension_built, path_under,
};
use systemprompt_api::services::server::routes::report_gateway_guards;
use systemprompt_api::services::server::routes::role::route_groups;
use systemprompt_extension::prelude::MissingDependency;
use systemprompt_extension::{ExtensionContext, RateBudget, RouteGroup};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::{StartupEvent, startup_channel};
use tower::ServiceExt;

fn empty(
    _ctx: &dyn ExtensionContext,
    _credential: GatewayCredential,
) -> Result<Router, MissingDependency> {
    Ok(Router::new())
}

const fn registration(
    extension: &'static str,
    path: &'static str,
    budget: RateBudget,
) -> GatewayRouteRegistration {
    GatewayRouteRegistration {
        extension,
        path,
        scope: "bridge_notifications",
        budget,
        factory: empty,
    }
}

fn admit(linked: &[GatewayRouteRegistration]) -> Result<(), MountError> {
    admit_registrations(linked, &builtin_gateway_prefixes())
}

const INBOX: GatewayRouteRegistration = registration(
    "notifications-http",
    ApiPaths::BRIDGE_NOTIFICATIONS,
    RateBudget::Notifications,
);

#[test]
fn the_listed_bridge_inbox_registration_is_admitted() {
    assert_eq!(
        CoreDeclaration::BridgeNotifications.base_path(),
        ApiPaths::BRIDGE_NOTIFICATIONS
    );
    assert!(admit(&[INBOX]).is_ok());
}

#[test]
fn an_unlisted_registration_is_refused() {
    let install = registration("install", "/v1/install", RateBudget::Gateway);
    assert!(matches!(
        admit(&[install]),
        Err(MountError::UnlistedGatewayRoute {
            extension: "install",
            path: "/v1/install",
        })
    ));

    let borrowed_id = registration(
        "notifications",
        ApiPaths::BRIDGE_NOTIFICATIONS,
        RateBudget::Notifications,
    );
    assert!(matches!(
        admit(&[borrowed_id]),
        Err(MountError::UnlistedGatewayRoute { .. })
    ));

    let other_path = registration(
        "notifications-http",
        "/v1/bridge/other",
        RateBudget::Notifications,
    );
    assert!(matches!(
        admit(&[other_path]),
        Err(MountError::UnlistedGatewayRoute { .. })
    ));
}

#[test]
fn a_listed_registration_with_another_budget_or_scope_is_refused() {
    let budget = registration(
        "notifications-http",
        ApiPaths::BRIDGE_NOTIFICATIONS,
        RateBudget::Gateway,
    );
    assert!(matches!(
        admit(&[budget]),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::RateLimit,
            ..
        })
    ));

    let scope = GatewayRouteRegistration {
        scope: "gateway",
        ..INBOX
    };
    assert!(matches!(
        admit(&[scope]),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::RateLimit,
            ..
        })
    ));
}

#[test]
fn overlapping_registrations_are_refused() {
    assert!(matches!(
        admit(&[INBOX, INBOX]),
        Err(MountError::GatewayRouteOverlap { .. })
    ));
}

#[test]
fn path_prefixes_are_segment_bounded() {
    assert!(path_under("/bridge/notifications", "/bridge/notifications"));
    assert!(path_under(
        "/bridge/notifications/read",
        "/bridge/notifications"
    ));
    assert!(!path_under(
        "/bridge/notificationsx",
        "/bridge/notifications"
    ));
    assert!(!path_under("/bridge/stream", "/bridge/notifications"));
}

fn built_on(role: NodeRole, extension: &str) -> bool {
    let groups = route_groups(role);
    extension_built(extension, &|group: RouteGroup| groups.contains(&group))
}

#[test]
fn routers_are_built_only_on_roles_that_could_serve_them() {
    for role in [NodeRole::All, NodeRole::Api] {
        assert!(built_on(role, "install"), "{role:?} serves Extensions");
    }
    for role in [NodeRole::Gateway, NodeRole::Worker, NodeRole::Runner] {
        assert!(
            !built_on(role, "install"),
            "{role:?} builds no install router"
        );
    }
    assert!(built_on(NodeRole::Worker, "agent_runs_api"));
    assert!(!built_on(NodeRole::Gateway, "agent_runs_api"));
    assert!(!built_on(NodeRole::Runner, "agent_runs_api"));
    assert!(
        !built_on(NodeRole::Gateway, "notifications-http"),
        "a gateway registration row does not build the extension's routers"
    );
    assert!(!built_on(NodeRole::Worker, "slack"));
}

#[test]
fn a_registration_overlapping_a_builtin_gateway_prefix_is_refused() {
    let builtin = builtin_gateway_prefixes();
    assert!(builtin.contains(&"/bridge/stream"));
    assert!(builtin.contains(&"/messages"));
    assert!(!builtin.iter().any(|prefix| {
        path_under("/bridge/notifications", prefix) || path_under(prefix, "/bridge/notifications")
    }));

    for shadowed in [
        "/bridge",
        "/bridge/notifications",
        "/bridge/notifications/read",
    ] {
        assert!(
            matches!(
                admit_registrations(&[INBOX], &[shadowed]),
                Err(MountError::GatewayRouteShadowsBuiltin {
                    path: ApiPaths::BRIDGE_NOTIFICATIONS,
                    builtin,
                }) if builtin == shadowed
            ),
            "{shadowed} must be refused"
        );
    }
}

async fn status(router: &axum::Router, uri: &str) -> StatusCode {
    router
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("infallible")
        .status()
}

#[tokio::test]
async fn a_registered_router_answers_404_outside_its_own_prefix() {
    let factory_routes = axum::Router::new()
        .route("/bridge/notifications", get(|| async { "inbox" }))
        .route("/bridge/notifications/read", get(|| async { "read" }))
        .route("/bridge/elsewhere", get(|| async { "outside" }))
        .route("/models", get(|| async { "models" }));
    let served = confined(
        factory_routes,
        vec![ApiPaths::gateway_relative(ApiPaths::BRIDGE_NOTIFICATIONS)],
    );

    assert_eq!(
        status(&served, "/bridge/notifications").await,
        StatusCode::OK
    );
    assert_eq!(
        status(&served, "/bridge/notifications/read").await,
        StatusCode::OK
    );
    assert_eq!(
        status(&served, "/bridge/elsewhere").await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(status(&served, "/models").await, StatusCode::NOT_FOUND);
}

#[test]
fn skipped_gateway_guards_reach_the_startup_summary() {
    let (events, mut received) = startup_channel();
    report_gateway_guards(&["credits", "forbidden"], Some(&events));
    match received.try_next() {
        Ok(Some(StartupEvent::Warning { message, context })) => {
            assert_eq!(message, "2 gateway guards not installed: owner disabled");
            assert_eq!(context.as_deref(), Some("credits, forbidden"));
        },
        other => panic!("expected a startup warning, got {other:?}"),
    }

    let (quiet, mut nothing) = startup_channel();
    report_gateway_guards(&[], Some(&quiet));
    drop(quiet);
    assert!(matches!(nothing.try_next(), Ok(None)));
}
