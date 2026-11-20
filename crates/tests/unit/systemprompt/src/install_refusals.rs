//! What an install member may not take: privileged shapes, core bases,
//! reserved CLI paths, gateway routes, core ids and cross-standing
//! dependencies (T3, T4, T5, T7, T13, T17, T21).

use systemprompt::analytics::entry::extension::AnalyticsHttpExtension;
use systemprompt::{CompositionError, reference};
use systemprompt_extension::cli::{CommandDecl, KernelGrant};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayPath, GatewayRouteDecl, RouteSet,
    RouteSpec, WebBase,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ContributionError, HandlerVerifier, RouteGroup,
};

use crate::cli_probe::{AnalyticsGroup, CloudGroup, ProbeGroup};
use crate::probe::{
    EXT_BASE, PLAIN, PLAIN_LIMITS, Probe, ProbeState, build, build_with, ok, plain_set,
};

fn single(spec: RouteSpec) -> RouteSet {
    RouteSet::at::<ProbeState>(EXT_BASE, RouteGroup::Extensions)
        .route(http::Method::POST, "/hook", ok, spec)
        .build()
}

fn refused_with(spec: RouteSpec) -> CompositionError {
    build_with(Probe::new("probe-install", move |c| {
        c.routes(single(spec));
    }))
    .expect_err("an install route with a privileged shape is refused")
}

#[test]
fn install_plain_route_at_extension_base_is_admitted() {
    let admitted = build_with(Probe::new("probe-install", |c| {
        c.routes(single(PLAIN));
    }))
    .expect("a plain install route at its own base is admitted");
    assert!(
        admitted
            .routes()
            .iter()
            .any(|set| set.owner.as_str() == "probe-install"
                && set.set.base() == BaseRef::Path(EXT_BASE))
    );
}

#[test]
fn install_handler_verified_policy_is_not_conferred() {
    let spec = RouteSpec::new(
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
        PLAIN_LIMITS,
    );
    assert!(matches!(
        refused_with(spec),
        CompositionError::NotConferred {
            allowance: Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            ..
        }
    ));
}

#[test]
fn install_handler_resolved_context_is_not_conferred() {
    let spec = RouteSpec::new(
        AuthzPolicy::user(),
        ContextKind::HandlerResolved,
        PLAIN_LIMITS,
    );
    assert!(matches!(
        refused_with(spec),
        CompositionError::NotConferred {
            allowance: Allowance::HandlerVerified(_),
            ..
        }
    ));
}

#[test]
fn install_agent_body_limit_is_not_conferred() {
    let mut limits = PLAIN_LIMITS;
    limits.body = BodyLimit::Agent;
    let spec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, limits);
    assert!(matches!(
        refused_with(spec),
        CompositionError::NotConferred {
            allowance: Allowance::BodyLimit,
            ..
        }
    ));
}

#[test]
fn install_ui_session_context_is_not_conferred() {
    let spec = RouteSpec::new(AuthzPolicy::user(), ContextKind::UserUi, PLAIN_LIMITS);
    assert!(matches!(
        refused_with(spec),
        CompositionError::NotConferred {
            allowance: Allowance::UiSession,
            ..
        }
    ));
}

#[test]
fn install_track_engagement_is_not_conferred() {
    let error = build_with(Probe::new("probe-install", |c| {
        c.routes(plain_set(BasePath::Web(WebBase::TrackEngagement), "/x"));
    }))
    .expect_err("the engagement web path is conferral-only");
    assert!(matches!(
        error,
        CompositionError::NotConferred {
            allowance: Allowance::WebPath,
            ..
        }
    ));
}

#[test]
fn install_closed_core_base_is_refused() {
    for base in [
        ApiBase::Content,
        ApiBase::Analytics,
        ApiBase::AdminNotifications,
    ] {
        let error = build_with(Probe::new("probe-install", move |c| {
            c.routes(plain_set(BasePath::Api(base), "/x"));
        }))
        .expect_err("a reserved core base is refused");
        assert!(
            matches!(error, CompositionError::ReservedPrefix { .. }),
            "{base:?}: {error:?}"
        );
    }
    for base in [ApiBase::Notifications, ApiBase::AgentRuns] {
        let error = build_with(Probe::new("probe-install", move |c| {
            c.routes(plain_set(BasePath::Api(base), "/x"));
        }))
        .expect_err("a closed core base is refused");
        assert!(
            matches!(
                error,
                CompositionError::NotConferred {
                    allowance: Allowance::Plain,
                    ..
                } | CompositionError::ReservedPrefix { .. }
            ),
            "{base:?}: {error:?}"
        );
    }
}

#[test]
fn impostor_without_analytics_http_gains_none_of_its_privileges() {
    let error = build(
        reference()
            .without::<AnalyticsHttpExtension>()
            .with(Probe::new("analytics-http", |_| {})),
    )
    .expect_err("an install cannot hold analytics-http's id");
    assert!(
        matches!(&error, CompositionError::ReservedId(id) if id.as_str() == "analytics-http"),
        "{error:?}"
    );
    let impostor = |f: fn(&mut systemprompt_extension::Contributions<'_>)| {
        build(
            reference()
                .without::<AnalyticsHttpExtension>()
                .with(Probe::new("analytics-impostor", f)),
        )
        .expect_err("an install in analytics-http's place gains nothing")
    };
    assert!(matches!(
        impostor(|c| {
            c.routes(plain_set(BasePath::Api(ApiBase::Analytics), "/stats"));
        }),
        CompositionError::ReservedPrefix { .. }
    ));
    assert!(matches!(
        impostor(|c| {
            c.routes(plain_set(BasePath::Web(WebBase::TrackEngagement), "/x"));
        }),
        CompositionError::NotConferred {
            allowance: Allowance::WebPath,
            ..
        }
    ));
    assert!(matches!(
        impostor(|c| {
            c.command(CommandDecl::group::<AnalyticsGroup>());
        }),
        CompositionError::ReservedCliPath {
            path: ["analytics"],
            ..
        }
    ));
}

#[test]
fn install_cloud_cli_and_grant_are_refused() {
    let error = build_with(Probe::new("probe-install", |c| {
        c.command(CommandDecl::group::<CloudGroup>().requesting(&[KernelGrant::CloudOperator]));
    }))
    .expect_err("the cloud CLI path is reserved");
    assert!(matches!(
        error,
        CompositionError::ReservedCliPath {
            path: ["cloud"],
            ..
        }
    ));

    let error = build_with(Probe::new("probe-install", |c| {
        c.command(CommandDecl::group::<ProbeGroup>().requesting(&[KernelGrant::CloudOperator]));
    }))
    .expect_err("a kernel grant is conferral-only");
    assert!(matches!(
        error,
        CompositionError::GrantNotConferred {
            grant: KernelGrant::CloudOperator,
            ..
        }
    ));
}

#[test]
fn install_requesting_a_reserved_path_grant_is_refused() {
    for grant in [
        KernelGrant::ServicesTree,
        KernelGrant::CallerSession,
        KernelGrant::BootProfile,
    ] {
        let error = build_with(Probe::new("probe-install", |c| {
            c.command(CommandDecl::group::<ProbeGroup>().requesting(&[grant]));
        }))
        .expect_err("an install command holds no grant");
        assert!(
            matches!(error, CompositionError::GrantNotConferred { grant: refused, .. } if refused == grant),
            "{error:?}"
        );
    }
}

#[test]
fn install_unreserved_cli_group_is_declared() {
    let declared = reference()
        .with(Probe::new("probe-install", |c| {
            c.command(CommandDecl::group::<ProbeGroup>());
        }))
        .declare()
        .expect("an unreserved install group is declared");
    assert!(
        declared
            .commands()
            .commands()
            .iter()
            .any(|cmd| cmd.owner.as_str() == "probe-install" && cmd.decl.path() == ["probe-tools"])
    );
}

#[test]
fn install_gateway_route_is_unlisted() {
    let error = build_with(Probe::new("probe-install", |c| {
        c.gateway_route(GatewayRouteDecl {
            limits: PLAIN_LIMITS,
            router: RouteSet::gateway::<ProbeState>(
                GatewayPath::BridgeNotifications,
                RouteGroup::Gateway,
            )
            .route(http::Method::GET, "/x", ok, PLAIN)
            .build(),
        });
    }))
    .expect_err("an install holds no gateway path");
    assert!(matches!(
        error,
        CompositionError::UnlistedGatewayRoute { .. }
    ));
}

#[test]
fn install_with_a_core_id_is_refused_as_reserved() {
    let error = reference()
        .with(Probe::new("analytics-http", |_| {}))
        .declare()
        .expect_err("a core id cannot be taken by an install");
    assert!(
        matches!(&error, CompositionError::ReservedId(id) if id.as_str() == "analytics-http"),
        "{error:?}"
    );
}

#[test]
fn two_installs_with_one_id_are_a_duplicate() {
    let error = reference()
        .with(Probe::new("probe-twin", |_| {}))
        .with(Probe::new("probe-twin", |_| {}))
        .declare()
        .expect_err("ids are unique");
    assert!(matches!(error, CompositionError::DuplicateId(id) if id.as_str() == "probe-twin"));
}

#[test]
fn install_depending_on_a_core_member_crosses_capabilities() {
    let error = reference()
        .with(Probe::new("probe-install", |c| {
            c.depends_on::<systemprompt::analytics::AnalyticsExtension>();
        }))
        .declare()
        .expect_err("an install may not depend on a core member");
    assert!(matches!(
        error,
        CompositionError::CrossCapabilityDependency { .. }
    ));
}

#[test]
fn invalid_extension_bases_are_refused_at_contribute() {
    for path in [
        "/ext/x",
        "/api/a/../b",
        "/api//x",
        "/api/x/",
        "/api/v1/contentx",
    ] {
        let error = build_with(Probe::new("probe-install", move |c| {
            c.routes(plain_set(BasePath::Api(ApiBase::Extension(path)), "/y"));
        }))
        .expect_err("an invalid extension base is refused");
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
}

#[test]
fn install_at_web_root_is_admitted_and_cannot_take_core_redirect() {
    let admitted = build_with(Probe::new("probe-install", |c| {
        c.routes(plain_set(BasePath::Web(WebBase::Root), "/page"));
    }))
    .expect("an install page at the web root is admitted");
    assert!(
        admitted
            .routes()
            .iter()
            .any(|set| set.owner.as_str() == "probe-install"
                && matches!(set.set.base(), BaseRef::Path(base) if base.is_web_root()))
    );

    let error = build_with(Probe::new("probe-install", |c| {
        c.routes(plain_set(BasePath::Web(WebBase::Root), "/r/{short_code}"));
    }))
    .expect_err("core's /r is reserved for content-http");
    assert!(
        matches!(error, CompositionError::ReservedPrefix { prefix: "/r", .. }),
        "{error:?}"
    );
}
