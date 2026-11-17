//! The composition confines a gateway route's defs to its claimed gateway
//! path with the same primitive the kernel's re-check uses: a def outside the
//! claim or under a built-in gateway prefix is refused, and every def of the
//! reference composition's gateway routes lies within its claim (K-SEC4-2).

use systemprompt_extension::RouteGroup;
use systemprompt_extension::router::{
    GatewayDefRefusal, GatewayPath, RouteSet, refused_gateway_def,
};

use crate::probe::{PLAIN, ProbeState, build, ok};

fn refusal(path: &'static str) -> Option<GatewayDefRefusal> {
    let set =
        RouteSet::gateway::<ProbeState>(GatewayPath::BridgeNotifications, RouteGroup::Gateway)
            .route(http::Method::GET, path, ok, PLAIN)
            .build();
    refused_gateway_def(GatewayPath::BridgeNotifications, &set.defs()[0])
}

#[test]
fn a_def_outside_the_claim_is_refused() {
    assert_eq!(
        refusal("/whoami"),
        Some(GatewayDefRefusal::OutsideClaim {
            path: "/whoami".to_owned(),
            claimed: "/bridge/notifications",
        })
    );
    assert_eq!(
        refusal("/bridge/notificationsx"),
        Some(GatewayDefRefusal::OutsideClaim {
            path: "/bridge/notificationsx".to_owned(),
            claimed: "/bridge/notifications",
        }),
        "a sibling sharing the claim's letters is outside it"
    );
}

#[test]
fn a_def_under_a_builtin_prefix_is_refused() {
    assert_eq!(
        refusal("/models/x"),
        Some(GatewayDefRefusal::UnderBuiltin {
            path: "/models/x".to_owned(),
            builtin: "/models",
        })
    );
    assert_eq!(
        refusal("/bridge/whoami"),
        Some(GatewayDefRefusal::UnderBuiltin {
            path: "/bridge/whoami".to_owned(),
            builtin: "/bridge/whoami",
        })
    );
}

#[test]
fn a_def_within_the_claim_is_admitted() {
    assert_eq!(refusal("/bridge/notifications/read"), None);
    assert_eq!(refusal("/bridge/notifications/whoami"), None);
    assert_eq!(refusal("/bridge/notifications"), None);
}

#[test]
fn every_reference_gateway_def_lies_within_its_claim() {
    let admitted = build(systemprompt::reference()).expect("the reference composition admits");
    assert!(!admitted.gateway_routes().is_empty());
    for route in admitted.gateway_routes() {
        let claimed = route
            .decl
            .path()
            .expect("a gateway route claims a gateway path");
        for def in route.decl.router.defs() {
            assert_eq!(
                refused_gateway_def(claimed, def),
                None,
                "{}",
                def.path.as_str()
            );
        }
    }
}
