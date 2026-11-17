//! A gateway route's every def is confined to the gateway path it claims: a
//! def outside the claim or under a built-in gateway prefix is refused by the
//! kernel's re-check of a forged handoff, and gateway defs join the
//! route-conflict check (K-SEC4-2).

use systemprompt_api::services::middleware::{MountError, recheck};
use systemprompt_extension::admitted::AdmittedGatewayRoute;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    Allowance, BaseRef, GatewayPath, GatewayRouteDecl, RouteMethod, RouteSpec,
};
use systemprompt_identifiers::ExtensionId;

use super::forge::{entry, forged, gateway_set};

fn bridge_route(paths: &[&'static str]) -> AdmittedGatewayRoute {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    let spec = RouteSpec::new(shape.shape.policy, shape.shape.context, shape.shape.limits);
    let routes: Vec<_> = paths
        .iter()
        .map(|path| (http::Method::POST, *path, spec))
        .collect();
    AdmittedGatewayRoute {
        owner: ExtensionId::new("notifications-http"),
        decl: GatewayRouteDecl {
            limits: shape.shape.limits,
            router: gateway_set(GatewayPath::BridgeNotifications, shape.group, &routes),
        },
    }
}

#[test]
fn a_def_outside_the_claimed_gateway_path_is_refused() {
    let outcome = recheck(&forged(vec![], vec![bridge_route(&["/whoami"])]));
    assert!(
        matches!(
            &outcome,
            Err(MountError::GatewayRouteOutsideClaim { path, claimed: "/bridge/notifications" })
                if path == "/whoami"
        ),
        "{outcome:?}"
    );
}

#[test]
fn a_def_under_a_builtin_gateway_prefix_is_refused() {
    for (path, builtin) in [
        ("/models/x", "/models"),
        ("/messages", "/messages"),
        ("/", "/"),
    ] {
        let outcome = recheck(&forged(vec![], vec![bridge_route(&[path])]));
        assert!(
            matches!(
                &outcome,
                Err(MountError::GatewayRouteUnderBuiltin { path: p, builtin: b })
                    if p == path && *b == builtin
            ),
            "{path}: {outcome:?}"
        );
    }
}

#[test]
fn a_def_within_the_claim_passes_the_recheck() {
    recheck(&forged(
        vec![],
        vec![bridge_route(&[
            "/bridge/notifications/read",
            "/bridge/notifications/whoami",
        ])],
    ))
    .expect("negative control: defs inside the claimed gateway path pass");
}

#[test]
fn two_gateway_defs_claiming_one_request_conflict() {
    let outcome = recheck(&forged(
        vec![],
        vec![
            bridge_route(&["/bridge/notifications/read"]),
            bridge_route(&["/bridge/notifications/read"]),
        ],
    ));
    assert!(
        matches!(
            &outcome,
            Err(MountError::RouteConflict { method: RouteMethod::One(m), path })
                if *m == http::Method::POST && path == "/v1/bridge/notifications/read"
        ),
        "{outcome:?}"
    );
    recheck(&forged(
        vec![],
        vec![
            bridge_route(&["/bridge/notifications/read"]),
            bridge_route(&["/bridge/notifications/seen"]),
        ],
    ))
    .expect("negative control: distinct gateway defs do not conflict");
}
