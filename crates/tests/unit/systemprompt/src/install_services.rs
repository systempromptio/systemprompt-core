//! A nested service is an install's whole-application mount: admitted at its
//! own extension base or the web root with the plain allowance, recorded as
//! one `ANY {prefix}/*` def with no operation schemas, refused under a
//! reserved prefix, and in conflict with every route its prefix covers.

use axum::Router;
use axum::routing::get;
use systemprompt::{CompositionError, reference};
use systemprompt_extension::router::{BasePath, RouteMethod, RouteSet, WebBase};
use systemprompt_extension::{Admitted, ContributionError, RouteGroup};

use crate::probe::{EXT_BASE, PLAIN, Probe, ProbeState, build, build_with, ok};

const ROOT: BasePath = BasePath::Web(WebBase::Root);

fn app() -> Router {
    Router::new().route("/", get(ok))
}

fn service_set(base: BasePath, prefix: &'static str) -> RouteSet {
    RouteSet::at::<ProbeState>(base, RouteGroup::Extensions)
        .service(prefix, app(), PLAIN)
        .build()
}

fn install_service(base: BasePath, prefix: &'static str) -> Result<Admitted, CompositionError> {
    build_with(Probe::new("probe-install", move |c| {
        c.routes(service_set(base, prefix));
    }))
}

fn service_defs(admitted: &Admitted) -> Vec<(String, &'static str, bool)> {
    admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == "probe-install")
        .flat_map(|set| set.set.defs())
        .map(|def| {
            (
                def.method.to_string(),
                def.path.as_str(),
                def.openapi.is_none(),
            )
        })
        .collect()
}

#[test]
fn an_install_service_is_admitted_as_one_opaque_def() {
    for base in [ROOT, EXT_BASE] {
        let admitted = install_service(base, "/dash").expect("an install service is admitted");
        assert_eq!(service_defs(&admitted), [("ANY".to_owned(), "/dash", true)]);
        let def = &admitted
            .routes()
            .iter()
            .find(|set| set.owner.as_str() == "probe-install")
            .expect("the install set is admitted")
            .set
            .defs()[0];
        assert!(def.is_service());
        assert_eq!(def.method, RouteMethod::Any);
    }
}

#[test]
fn a_service_under_a_reserved_prefix_is_refused() {
    let error = install_service(ROOT, "/api/v1/content")
        .expect_err("a service under a reserved prefix is refused");
    assert!(
        matches!(
            error,
            CompositionError::ReservedPrefix {
                prefix: "/api/v1/content",
                ..
            }
        ),
        "{error:?}"
    );
    install_service(ROOT, "/api/v1/probe")
        .expect("negative control: an unreserved prefix at the web root is admitted");
}

#[test]
fn one_route_spelled_with_two_parameter_names_is_refused_at_admission() {
    let member = |id: &'static str, path: &'static str| {
        Probe::new(id, move |c| {
            c.routes(
                RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                    .route(http::Method::GET, path, ok, PLAIN)
                    .build(),
            );
        })
    };
    let error = build(
        reference()
            .with(member("probe-by-id", "/probe-items/{id}"))
            .with(member("probe-by-slug", "/probe-items/{slug}")),
    )
    .expect_err("one route under two parameter names cannot share the router");
    assert!(
        matches!(&error, CompositionError::RouteConflict { path, .. } if path == "/probe-items/{slug}"),
        "{error:?}"
    );
    build(
        reference()
            .with(member("probe-by-id", "/probe-items/{id}"))
            .with(member("probe-by-slug-detail", "/probe-items/{slug}/detail")),
    )
    .expect("negative control: a deeper route naming its parameter differently is admitted");
}

#[test]
fn a_service_prefix_covering_a_listed_route_conflicts() {
    let error = install_service(ROOT, "/r").expect_err("the prefix is core's redirect path");
    assert!(
        matches!(
            &error,
            CompositionError::ReservedPrefix { prefix: "/r", .. }
        ),
        "{error:?}"
    );

    let error = build_with(Probe::new("probe-install", |c| {
        c.routes(
            RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                .route(http::Method::GET, "/dash/home", ok, PLAIN)
                .service("/dash", app(), PLAIN)
                .build(),
        );
    }))
    .expect_err("a service covering its own set's route conflicts");
    assert!(
        matches!(&error, CompositionError::RouteConflict { path, .. } if path == "/dash"),
        "{error:?}"
    );

    build_with(Probe::new("probe-install", |c| {
        c.routes(
            RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                .route(http::Method::GET, "/dashboard", ok, PLAIN)
                .service("/dash", app(), PLAIN)
                .build(),
        );
    }))
    .expect("negative control: a sibling path sharing the prefix's letters is not covered");
}

#[test]
fn a_service_prefix_must_be_a_literal_below_the_root() {
    for prefix in ["/", "/dash/{id}", "/dash/*rest"] {
        let error = install_service(ROOT, prefix).expect_err("a non-literal prefix is refused");
        assert!(
            matches!(
                &error,
                CompositionError::Contribution {
                    source: ContributionError::InvalidPath(p),
                    ..
                } if *p == prefix
            ),
            "{prefix}: {error:?}"
        );
    }
}
