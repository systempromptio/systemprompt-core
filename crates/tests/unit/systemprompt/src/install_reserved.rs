//! An install's every def is compared, by its full path, with the kernel's
//! reserved paths at admission: a literal, a catch-all, or a parameter def
//! that reaches inside a kernel or core namespace, and a service prefix or
//! extension base that covers one, are refused; a parameter segment at the
//! place of a reserved literal, as deep as that entry, is admitted (the
//! literal takes precedence); the core owners still hold their own paths.

use axum::Router;
use axum::routing::get;
use systemprompt::analytics::entry::extension::AnalyticsHttpExtension;
use systemprompt::{CompositionError, reference};
use systemprompt_extension::router::{ApiBase, BasePath, RouteSet, WebBase};
use systemprompt_extension::{Admitted, ContributionError, RouteGroup};

use crate::probe::{PLAIN, Probe, ProbeState, build, build_with, ok, plain_set};

const ROOT: BasePath = BasePath::Web(WebBase::Root);

fn install_page(path: &'static str) -> Result<Admitted, CompositionError> {
    build_with(Probe::new("probe-install", move |c| {
        c.routes(plain_set(ROOT, path));
    }))
}

fn install_service(prefix: &'static str) -> Result<Admitted, CompositionError> {
    build_with(Probe::new("probe-install", move |c| {
        c.routes(
            RouteSet::at::<ProbeState>(ROOT, RouteGroup::Extensions)
                .service(prefix, Router::new().route("/", get(ok)), PLAIN)
                .build(),
        );
    }))
}

fn refused_at(result: Result<Admitted, CompositionError>) -> &'static str {
    match result {
        Err(CompositionError::ReservedPrefix { prefix, .. }) => prefix,
        Err(error) => panic!("expected a reserved-path refusal, got {error:?}"),
        Ok(_) => panic!("expected a reserved-path refusal, the def was admitted"),
    }
}

#[test]
fn an_install_literal_under_a_kernel_namespace_is_refused() {
    for (path, reserved) in [
        ("/api/v1/mcp/github/mcp", "/api/v1/mcp"),
        ("/.well-known/openid-configuration", "/.well-known"),
        ("/.well-known/x", "/.well-known"),
        ("/v1/x", "/v1"),
        ("/v1/messages/extra", "/v1"),
        ("/api/public/gateway/x", "/api/public/gateway"),
        ("/api/v1/core/oauth/x", "/api/v1/core"),
        ("/api/v1/notifications/x", "/api/v1/notifications"),
        ("/api/v1/admin/notifications/x", "/api/v1/admin"),
        ("/api/v1", "/api/v1"),
        ("/livez", "/livez"),
        ("/auth/link-passkey", "/auth/link-passkey"),
        ("/r/abc", "/r"),
        ("/track/engagement/x", "/track"),
    ] {
        assert_eq!(refused_at(install_page(path)), reserved, "{path}");
    }
    for path in [
        "/api/v1/probe/x",
        "/v1x/a",
        "/well-known/x",
        "/api/public/hooks",
        "/livez/detail",
        "/rx",
        "/page",
    ] {
        install_page(path).unwrap_or_else(|error| {
            panic!("negative control: {path} lies under no reserved path: {error:?}")
        });
    }
}

#[test]
fn an_install_parameter_or_catch_all_reaching_a_kernel_namespace_is_refused() {
    assert_eq!(
        refused_at(install_page("/api/{version}/mcp/x")),
        "/api/v1/mcp"
    );
    assert_eq!(
        refused_at(install_page("/api/public/{short_code}/{rest}")),
        "/api/public/gateway"
    );
    for path in [
        "/{section}/agent-card.json",
        "/{*path}",
        "/api/{*rest}",
        "/api/public/{*rest}",
    ] {
        refused_at(install_page(path));
    }
    for path in [
        "/pages/{slug}",
        "/pages/{*rest}",
        "/api/public/s/{short_code}",
        "/api/public/{short_code}",
        "/{slug}",
    ] {
        install_page(path).unwrap_or_else(|error| {
            panic!("negative control: {path} reaches no reserved path: {error:?}")
        });
    }
}

#[test]
fn an_install_service_covering_a_kernel_namespace_is_refused() {
    for (prefix, reserved) in [
        ("/api", "/api/v1"),
        ("/api/v1", "/api/v1"),
        ("/api/public", "/api/public/gateway"),
        ("/.well-known", "/.well-known"),
        ("/v1", "/v1"),
    ] {
        assert_eq!(refused_at(install_service(prefix)), reserved, "{prefix}");
    }
    assert!(matches!(
        install_service("/"),
        Err(CompositionError::Contribution {
            source: ContributionError::InvalidPath("/"),
            ..
        })
    ));
    for prefix in ["/apix", "/api/public-ui", "/dash"] {
        install_service(prefix).unwrap_or_else(|error| {
            panic!("negative control: a service at {prefix} covers no reserved path: {error:?}")
        });
    }
}

#[test]
fn an_extension_base_covering_a_kernel_namespace_is_refused() {
    for path in ["/api/v1", "/api/public"] {
        let error = build_with(Probe::new("probe-install", move |c| {
            c.routes(plain_set(BasePath::Api(ApiBase::Extension(path)), "/x"));
        }))
        .expect_err("a base above a reserved path is refused");
        assert!(
            matches!(
                &error,
                CompositionError::Contribution {
                    source: ContributionError::InvalidBase(p),
                    ..
                } if *p == path
            ),
            "{path}: {error:?}"
        );
    }
    for path in ["/api/v1/probe", "/api/publicx"] {
        build_with(Probe::new("probe-install", move |c| {
            c.routes(plain_set(BasePath::Api(ApiBase::Extension(path)), "/x"));
        }))
        .unwrap_or_else(|error| {
            panic!("negative control: {path} covers no reserved path: {error:?}")
        });
    }
}

#[test]
fn the_core_owners_hold_their_reserved_paths() {
    let admitted = build(reference()).expect("the reference composition admits");
    let held = |owner: &str, base: BasePath| {
        admitted.routes().iter().any(|set| {
            set.owner.as_str() == owner
                && set.set.base() == systemprompt_extension::router::BaseRef::Path(base)
        })
    };
    assert!(
        held("content-http", ROOT),
        "content-http holds /r at the web root"
    );
    assert!(held(
        "analytics-http",
        BasePath::Web(WebBase::TrackEngagement)
    ));
    assert!(
        held(
            "notifications-http",
            BasePath::Api(ApiBase::AdminNotifications)
        ),
        "notifications-http holds its entry nested in the kernel's /api/v1/admin"
    );

    let error = build(
        reference()
            .without::<AnalyticsHttpExtension>()
            .with(Probe::new("analytics-impostor", |c| {
                c.routes(plain_set(ROOT, "/track/engagement/x"));
            })),
    )
    .expect_err("an install in analytics-http's place does not hold its reserved path");
    assert!(
        matches!(
            error,
            CompositionError::ReservedPrefix {
                prefix: "/track",
                ..
            }
        ),
        "{error:?}"
    );
}
