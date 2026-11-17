//! The reserved list cannot rot: every kernel and core route in the route
//! baseline lies under a reserved path its owner may route and an install
//! may not, and so does every privileged shape's base and pinned route. A
//! route mounted outside the list fails here once the baseline records it.

use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES};
use systemprompt_extension::router::{
    BasePath, BaseRef, RouteDef, RouteMethod, RoutePath, WebBase, refused_reservation,
};

use crate::probe::PLAIN;

const BASELINE: &str = include_str!("../fixtures/baseline/routes.tsv");
const ROOT: BaseRef = BaseRef::Path(BasePath::Web(WebBase::Root));
const STATIC_FALLBACK: &str = "/{*fallback}";

fn def(path: &'static str) -> RouteDef {
    RouteDef {
        method: RouteMethod::One(http::Method::GET),
        path: RoutePath::parse(path).unwrap_or_else(|error| panic!("{path}: {error:?}")),
        spec: PLAIN,
        openapi: None,
    }
}

#[test]
fn every_baseline_route_is_reserved_for_its_owner() {
    let mut checked = 0;
    for line in BASELINE.lines().skip(1) {
        let columns: Vec<&'static str> = line.split('\t').collect();
        let (path, group, owner) = (columns[1], columns[6], columns[7]);
        if path == STATIC_FALLBACK {
            assert_eq!((group, owner), ("Static", "kernel"), "the root fallback is the kernel's");
            continue;
        }
        let path = if path.len() > 1 {
            path.trim_end_matches('/')
        } else {
            path
        };
        let route = def(path);
        assert!(
            refused_reservation(ROOT, &route, None).is_some(),
            "{path} ({owner}) lies outside the reserved paths: an install could route it"
        );
        if group != "Gateway" {
            let holder = CoreId::for_id(owner).unwrap_or_else(|| panic!("{owner} is not a core id"));
            assert_eq!(
                refused_reservation(ROOT, &route, Some(holder)),
                None,
                "{path} is reserved away from its owner {owner}"
            );
        }
        checked += 1;
    }
    assert!(checked > 250, "the baseline was read: {checked} routes");
    assert_eq!(
        refused_reservation(ROOT, &def("/api/probe/x"), None),
        None,
        "negative control: an unreserved path is open to an install"
    );
}

#[test]
fn the_unmounted_api_constants_are_reserved_for_their_owners() {
    for (path, owner) in [
        ("/api/v1/auth/me", CoreId::Kernel),
        ("/api/v1/meta", CoreId::Kernel),
        ("/api/v1/meta/probe", CoreId::Kernel),
        ("/api/v1/tenants", CoreId::Cloud),
        ("/api/v1/tenants/probe", CoreId::Cloud),
    ] {
        let route = def(path);
        assert!(
            refused_reservation(ROOT, &route, None).is_some(),
            "{path} is open to an install"
        );
        assert_eq!(
            refused_reservation(ROOT, &route, Some(owner)),
            None,
            "{path} is reserved away from {owner:?}"
        );
        assert!(
            refused_reservation(ROOT, &route, Some(CoreId::ContentHttp)).is_some(),
            "{path} is open to a core member that does not own it"
        );
    }
}

#[test]
fn every_privileged_shape_is_reserved_for_its_owner() {
    for shape in PRIVILEGED_SHAPES {
        let BaseRef::Path(base) = shape.base else {
            continue;
        };
        let paths: Vec<&'static str> = shape
            .routes
            .map_or_else(|| vec!["/probe"], |routes| routes.iter().map(|(_, path)| *path).collect());
        for path in paths {
            let route = def(path);
            assert!(
                refused_reservation(shape.base, &route, None).is_some(),
                "{base:?} {path} is open to an install"
            );
            assert_eq!(
                refused_reservation(shape.base, &route, Some(shape.owner)),
                None,
                "{base:?} {path} is reserved away from {:?}",
                shape.owner
            );
        }
    }
}
