//! A handoff built through the hidden facade constructor gains nothing
//! beyond a shape the kernel constants list, under the core id they list it
//! for. The kernel's `recheck` judges the handoff with no database (T2, T16
//! and T18 kernel half, T9, T14, T15, T17 mount half).

use systemprompt_api::services::middleware::{MountError, recheck};

use super::forge::{PLAIN_LIMITS, analytics_set, entry, forged, gateway_set, routed, set, spec_of};
use systemprompt_extension::admitted::AdmittedGatewayRoute;
use systemprompt_extension::reserved::{CoreId, PRIVILEGED_SHAPES};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayPath, GatewayRouteDecl, RouteLimits,
    RouteSpec,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandleId, HandlerVerifier, RateBudget, RateLimit, RouteGroup,
};
use systemprompt_identifiers::ExtensionId;

#[test]
fn handler_verified_at_an_unlisted_base_is_refused() {
    let base = BasePath::Api(ApiBase::Extension("/api/probe"));
    let spec = RouteSpec::new(
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
        PLAIN_LIMITS,
    );
    let admitted = forged(
        vec![routed(
            "probe-forger",
            set(
                base,
                RouteGroup::Messaging,
                &[(http::Method::POST, "/events", spec)],
            ),
            vec![],
        )],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::UnlistedPrivilege {
            allowance: Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            ..
        })
    ));
}

#[test]
fn listed_privilege_under_a_non_core_id_is_an_owner_mismatch() {
    let admitted = forged(
        vec![routed(
            "probe-forger",
            analytics_set(),
            vec![HandleId::AnalyticsBroadcaster],
        )],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::PrivilegeOwnerMismatch {
            expected: CoreId::AnalyticsHttp,
            ..
        })
    ));
}

#[test]
fn one_id_holding_two_core_ids_is_an_owner_mismatch() {
    let content = entry(
        CoreId::ContentHttp,
        BaseRef::Path(BasePath::Api(ApiBase::Content)),
        Allowance::ReservedPrefix,
    );
    let content_set = set(
        BasePath::Api(ApiBase::Content),
        content.group,
        &[(http::Method::GET, "/probe", spec_of(content))],
    );
    let admitted = forged(
        vec![
            routed("content-http", content_set, vec![]),
            routed("content-http", analytics_set(), vec![]),
        ],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::PrivilegeOwnerMismatch {
            expected: CoreId::AnalyticsHttp,
            ..
        })
    ));
}

#[test]
fn listed_privilege_under_its_core_id_passes_the_recheck() {
    let admitted = forged(
        vec![routed(
            "analytics-http",
            analytics_set(),
            vec![HandleId::AnalyticsBroadcaster],
        )],
        vec![],
    );
    assert!(recheck(&admitted).is_ok(), "negative control");
}

#[test]
fn slack_shape_is_listed_only_in_a_slack_build() {
    let base = BasePath::Api(ApiBase::Slack);
    let spec = RouteSpec::new(
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
        RouteLimits {
            rate: RateLimit::new("agents", RateBudget::Agents),
            body: BodyLimit::Default,
        },
    );
    let admitted = forged(
        vec![routed(
            "slack",
            set(
                base,
                RouteGroup::Messaging,
                &[(http::Method::POST, "/events", spec)],
            ),
            vec![],
        )],
        vec![],
    );
    let listed = PRIVILEGED_SHAPES.iter().any(|e| e.owner == CoreId::Slack);
    let result = recheck(&admitted);
    if listed {
        assert!(result.is_ok(), "a slack build lists the slack shape");
    } else {
        assert!(
            matches!(result, Err(MountError::ReservedPrefix("/api/v1/slack"))),
            "{result:?}"
        );
    }
}

fn drifted(shape: RouteSpec) -> Vec<(&'static str, RouteSpec)> {
    let mut out = Vec::new();
    let policy = if shape.policy == AuthzPolicy::public() {
        AuthzPolicy::admin()
    } else {
        AuthzPolicy::public()
    };
    out.push(("policy", RouteSpec { policy, ..shape }));
    let context = if shape.context == ContextKind::A2a {
        ContextKind::Mcp
    } else {
        ContextKind::A2a
    };
    out.push(("context", RouteSpec { context, ..shape }));
    let mut limits = shape.limits;
    limits.rate.budget = if limits.rate.budget == RateBudget::McpRegistry {
        RateBudget::Artifacts
    } else {
        RateBudget::McpRegistry
    };
    out.push(("rate budget", RouteSpec { limits, ..shape }));
    let mut limits = shape.limits;
    limits.rate.scope = "drifted";
    out.push(("rate scope", RouteSpec { limits, ..shape }));
    let mut limits = shape.limits;
    limits.rate.bearer_window = !limits.rate.bearer_window;
    out.push(("bearer window", RouteSpec { limits, ..shape }));
    let mut limits = shape.limits;
    limits.body = if limits.body == BodyLimit::Agent {
        BodyLimit::Default
    } else {
        BodyLimit::Agent
    };
    out.push(("body limit", RouteSpec { limits, ..shape }));
    out
}

#[test]
fn every_listed_shape_refuses_drift_in_any_field_or_group() {
    for listed in PRIVILEGED_SHAPES {
        let BaseRef::Path(base) = listed.base else {
            continue;
        };
        if matches!(listed.allowance, Allowance::KernelHandle(_)) {
            continue;
        }
        let (method, path) = listed
            .routes
            .and_then(|routes| routes.first().cloned())
            .unwrap_or((http::Method::GET, "/probe"));
        let exact = forged(
            vec![routed(
                listed.owner.id(),
                set(
                    base,
                    listed.group,
                    &[(method.clone(), path, spec_of(listed))],
                ),
                vec![],
            )],
            vec![],
        );
        assert!(
            recheck(&exact).is_ok(),
            "negative control: {:?} at {base:?} in its listed shape",
            listed.owner
        );
        for (field, spec) in drifted(spec_of(listed)) {
            let admitted = forged(
                vec![routed(
                    listed.owner.id(),
                    set(base, listed.group, &[(method.clone(), path, spec)]),
                    vec![],
                )],
                vec![],
            );
            let outcome = recheck(&admitted);
            assert!(
                matches!(outcome, Err(MountError::UnlistedPrivilege { .. })),
                "{:?} at {base:?} with drifted {field} must be refused: {outcome:?}",
                listed.owner
            );
        }
        let group = if listed.group == RouteGroup::Discovery {
            RouteGroup::Oauth
        } else {
            RouteGroup::Discovery
        };
        let moved = forged(
            vec![routed(
                listed.owner.id(),
                set(base, group, &[(method.clone(), path, spec_of(listed))]),
                vec![],
            )],
            vec![],
        );
        assert!(
            matches!(recheck(&moved), Err(MountError::UnlistedPrivilege { .. })),
            "{:?} at {base:?} under another group must be refused",
            listed.owner
        );
    }
}

#[test]
fn only_the_kernel_agent_proxy_lists_the_agent_body_limit() {
    let agent: Vec<_> = PRIVILEGED_SHAPES
        .iter()
        .filter(|e| e.shape.limits.body == BodyLimit::Agent || e.allowance == Allowance::BodyLimit)
        .collect();
    assert_eq!(agent.len(), 1);
    assert_eq!(agent[0].owner, CoreId::Kernel);
    assert_eq!(agent[0].base, BaseRef::Path(BasePath::Api(ApiBase::Agents)));
    assert_eq!(agent[0].allowance, Allowance::BodyLimit);
}

#[test]
fn content_generate_with_the_public_shape_or_a_changed_group_is_refused() {
    let content = BasePath::Api(ApiBase::Content);
    let public = entry(
        CoreId::ContentHttp,
        BaseRef::Path(content),
        Allowance::ReservedPrefix,
    );
    let admitted = forged(
        vec![routed(
            "content-http",
            set(
                content,
                public.group,
                &[(http::Method::POST, "/links/generate", spec_of(public))],
            ),
            vec![],
        )],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::UnlistedPrivilege { .. })
    ));

    let admitted = forged(
        vec![routed(
            "content-http",
            set(
                content,
                RouteGroup::Messaging,
                &[(http::Method::GET, "/probe", spec_of(public))],
            ),
            vec![],
        )],
        vec![],
    );
    assert!(matches!(
        recheck(&admitted),
        Err(MountError::UnlistedPrivilege { .. })
    ));
}

#[test]
fn reserved_extension_base_is_refused_at_mount() {
    let base = BasePath::Api(ApiBase::Extension("/api/v1/contentx"));
    let admitted = forged(
        vec![routed(
            "probe-forger",
            set(
                base,
                RouteGroup::Extensions,
                &[(
                    http::Method::GET,
                    "/x",
                    RouteSpec::new(AuthzPolicy::user(), ContextKind::User, PLAIN_LIMITS),
                )],
            ),
            vec![],
        )],
        vec![],
    );
    let outcome = recheck(&admitted);
    assert!(
        matches!(outcome, Err(MountError::ReservedPrefix(_))),
        "{outcome:?}"
    );
}

fn bridge_route(limits: RouteLimits) -> AdmittedGatewayRoute {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    let spec = RouteSpec::new(shape.shape.policy, shape.shape.context, limits);
    AdmittedGatewayRoute {
        owner: ExtensionId::new("notifications-http"),
        decl: GatewayRouteDecl {
            limits,
            router: gateway_set(
                GatewayPath::BridgeNotifications,
                shape.group,
                &[(http::Method::GET, "/", spec)],
            ),
        },
    }
}

#[test]
fn bridge_gateway_route_needs_the_bearer_window() {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    assert!(
        shape.shape.limits.rate.bearer_window,
        "the listed shape carries the bearer window"
    );
    let mut without = shape.shape.limits;
    without.rate = RateLimit::new(without.rate.scope, without.rate.budget);
    assert!(matches!(
        recheck(&forged(vec![], vec![bridge_route(without)])),
        Err(MountError::UnlistedPrivilege {
            allowance: Allowance::GatewayRoute,
            ..
        })
    ));
    assert!(
        recheck(&forged(vec![], vec![bridge_route(shape.shape.limits)])).is_ok(),
        "negative control: the listed limits pass the recheck"
    );
}

#[test]
fn bridge_gateway_route_under_a_non_core_id_is_an_owner_mismatch() {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    let mut route = bridge_route(shape.shape.limits);
    route.owner = ExtensionId::new("probe-forger");
    assert!(matches!(
        recheck(&forged(vec![], vec![route])),
        Err(MountError::PrivilegeOwnerMismatch {
            expected: CoreId::NotificationsHttp,
            ..
        })
    ));
}

#[test]
fn a_gateway_route_built_at_a_path_base_is_refused() {
    let shape = entry(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    let limits = shape.shape.limits;
    let spec = RouteSpec::new(shape.shape.policy, shape.shape.context, limits);
    let base = BasePath::Api(ApiBase::Extension("/api/bridge-probe"));
    let route = AdmittedGatewayRoute {
        owner: ExtensionId::new("notifications-http"),
        decl: GatewayRouteDecl {
            limits,
            router: set(base, shape.group, &[(http::Method::GET, "/", spec)]),
        },
    };
    assert!(matches!(
        recheck(&forged(vec![], vec![route])),
        Err(MountError::RouteSetKind(BaseRef::Path(at))) if at == base
    ));
}
