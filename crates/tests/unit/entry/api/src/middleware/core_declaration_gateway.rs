//! Gateway route registrations are core declarations: only a registration a
//! `Gateway`-group row lists by extension id, exact path, scope and budget is
//! admitted, registrations may not overlap, and an extension's routers are
//! built only on a role that could serve them.

use axum::Router;
use systemprompt_api::routes::gateway::registered::{
    GatewayCredential, GatewayRouteRegistration, admit_registrations,
};
use systemprompt_api::services::middleware::{
    CoreDeclaration, CoreShapeField, MountError, extension_built, path_under,
};
use systemprompt_api::services::server::routes::role::route_groups;
use systemprompt_extension::prelude::MissingDependency;
use systemprompt_extension::{ExtensionContext, RateBudget, RouteGroup};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_models::modules::ApiPaths;

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
    assert!(admit_registrations(&[INBOX]).is_ok());
}

#[test]
fn an_unlisted_registration_is_refused() {
    let install = registration("install", "/v1/install", RateBudget::Gateway);
    assert!(matches!(
        admit_registrations(&[install]),
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
        admit_registrations(&[borrowed_id]),
        Err(MountError::UnlistedGatewayRoute { .. })
    ));

    let other_path = registration(
        "notifications-http",
        "/v1/bridge/other",
        RateBudget::Notifications,
    );
    assert!(matches!(
        admit_registrations(&[other_path]),
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
        admit_registrations(&[budget]),
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
        admit_registrations(&[scope]),
        Err(MountError::CoreDeclarationMismatch {
            field: CoreShapeField::RateLimit,
            ..
        })
    ));
}

#[test]
fn overlapping_registrations_are_refused() {
    assert!(matches!(
        admit_registrations(&[INBOX, INBOX]),
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
