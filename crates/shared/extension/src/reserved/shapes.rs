//! The privileged route shapes, in the order of the reference composition's
//! conferral table; the Slack and Teams shapes exist only with their
//! features.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::Method;

use super::{CoreId, PrivilegedShape};
use crate::authz::{AuthzPolicy, HandlerVerifier};
use crate::contribute::HandleId;
use crate::protection::{ContextKind, RateBudget, RateLimit, RouteGroup};
use crate::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayPath, RouteLimits, RouteShape, WebBase,
};

const fn limits(rate: RateLimit) -> RouteLimits {
    RouteLimits {
        rate,
        body: BodyLimit::Default,
    }
}

const fn shape(policy: AuthzPolicy, context: ContextKind, limits: RouteLimits) -> RouteShape {
    RouteShape {
        policy,
        context,
        limits,
    }
}

const CONTENT_LIMITS: RouteLimits = limits(RateLimit::new("content", RateBudget::Content));
const NOTIFICATIONS_LIMITS: RouteLimits =
    limits(RateLimit::new("notifications", RateBudget::Notifications));
const AGENTS_RATE: RateLimit = RateLimit::new("agents", RateBudget::Agents);

const fn agent_user(base: ApiBase, rate: RateLimit) -> PrivilegedShape {
    PrivilegedShape {
        owner: CoreId::AgentHttp,
        base: BaseRef::Path(BasePath::Api(base)),
        group: RouteGroup::Agent,
        allowance: Allowance::ReservedPrefix,
        shape: shape(AuthzPolicy::user(), ContextKind::User, limits(rate)),
        routes: None,
    }
}

const CONTENT_PUBLIC: RouteShape =
    shape(AuthzPolicy::public(), ContextKind::Public, CONTENT_LIMITS);
const CONTENT_USER: RouteShape = shape(AuthzPolicy::user(), ContextKind::User, CONTENT_LIMITS);
const ANALYTICS_ADMIN: RouteShape = shape(AuthzPolicy::admin(), ContextKind::User, CONTENT_LIMITS);

pub const CONTENT_GENERATE_ROUTES: &[(Method, &str)] = &[(Method::POST, "/links/generate")];
pub const CONTENT_REDIRECT_ROUTES: &[(Method, &str)] = &[(Method::GET, "/r/{short_code}")];
pub const MCP_REGISTRY_ROUTES: &[(Method, &str)] = &[(Method::GET, "/")];
pub const AGENT_REGISTRY_ROUTES: &[(Method, &str)] = &[(Method::GET, "/")];

const BRAIN_LIMITS: RouteLimits = limits(RateLimit::new("brain", RateBudget::Brain));

const ALWAYS: [PrivilegedShape; 19] = [
    PrivilegedShape {
        owner: CoreId::ContentHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Content)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: CONTENT_PUBLIC,
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::ContentHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Content)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: CONTENT_USER,
        routes: Some(CONTENT_GENERATE_ROUTES),
    },
    PrivilegedShape {
        owner: CoreId::ContentHttp,
        base: BaseRef::Path(BasePath::Web(WebBase::Root)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::Plain,
        shape: CONTENT_PUBLIC,
        routes: Some(CONTENT_REDIRECT_ROUTES),
    },
    PrivilegedShape {
        owner: CoreId::AnalyticsHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Analytics)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: ANALYTICS_ADMIN,
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::AnalyticsHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Analytics)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::KernelHandle(HandleId::AnalyticsBroadcaster),
        shape: ANALYTICS_ADMIN,
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::AnalyticsHttp,
        base: BaseRef::Path(BasePath::Web(WebBase::TrackEngagement)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::WebPath,
        shape: CONTENT_PUBLIC,
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::NotificationsHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Notifications)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::Plain,
        shape: shape(AuthzPolicy::user(), ContextKind::User, NOTIFICATIONS_LIMITS),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::NotificationsHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::AdminNotifications)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: shape(
            AuthzPolicy::admin(),
            ContextKind::User,
            NOTIFICATIONS_LIMITS,
        ),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::NotificationsHttp,
        base: BaseRef::Gateway(GatewayPath::BridgeNotifications),
        group: RouteGroup::Gateway,
        allowance: Allowance::GatewayRoute,
        shape: shape(
            AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential),
            ContextKind::HandlerResolved,
            limits(
                RateLimit::new("bridge_notifications", RateBudget::Notifications)
                    .with_bearer_window(),
            ),
        ),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::AgentRunsApi,
        base: BaseRef::Path(BasePath::Api(ApiBase::AgentRuns)),
        group: RouteGroup::Agent,
        allowance: Allowance::Plain,
        shape: shape(
            AuthzPolicy::user(),
            ContextKind::User,
            limits(RateLimit::new("agent_runs", RateBudget::Tasks)),
        ),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::McpHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::McpRegistry)),
        group: RouteGroup::McpAndStream,
        allowance: Allowance::ReservedPrefix,
        shape: shape(
            AuthzPolicy::public(),
            ContextKind::Public,
            limits(RateLimit::new("mcp_registry", RateBudget::McpRegistry)),
        ),
        routes: Some(MCP_REGISTRY_ROUTES),
    },
    agent_user(
        ApiBase::CoreContexts,
        RateLimit::new("contexts", RateBudget::Contexts),
    ),
    agent_user(
        ApiBase::CoreTasks,
        RateLimit::new("tasks", RateBudget::Tasks),
    ),
    agent_user(
        ApiBase::CoreArtifacts,
        RateLimit::new("artifacts", RateBudget::Artifacts),
    ),
    PrivilegedShape {
        owner: CoreId::AgentHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::AgentsRegistry)),
        group: RouteGroup::Agent,
        allowance: Allowance::ReservedPrefix,
        shape: shape(
            AuthzPolicy::public(),
            ContextKind::Public,
            limits(RateLimit::new("agent_registry", RateBudget::AgentRegistry)),
        ),
        routes: Some(AGENT_REGISTRY_ROUTES),
    },
    PrivilegedShape {
        owner: CoreId::Kernel,
        base: BaseRef::Path(BasePath::Api(ApiBase::Agents)),
        group: RouteGroup::Agent,
        allowance: Allowance::BodyLimit,
        shape: shape(
            AuthzPolicy::authenticated(),
            ContextKind::A2a,
            RouteLimits {
                rate: AGENTS_RATE,
                body: BodyLimit::Agent,
            },
        ),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::MarketplaceHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Marketplace)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: shape(
            AuthzPolicy::public(),
            ContextKind::Public,
            limits(RateLimit::new("marketplace", RateBudget::Content)),
        ),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::BrainHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Brain)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: shape(AuthzPolicy::user(), ContextKind::User, BRAIN_LIMITS),
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::BrainHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::AdminBrain)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: shape(AuthzPolicy::admin(), ContextKind::User, BRAIN_LIMITS),
        routes: None,
    },
];

#[cfg(feature = "slack")]
const SLACK: PrivilegedShape = PrivilegedShape {
    owner: CoreId::Slack,
    base: BaseRef::Path(BasePath::Api(ApiBase::Slack)),
    group: RouteGroup::Messaging,
    allowance: Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
    shape: shape(
        AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
        ContextKind::HandlerResolved,
        limits(AGENTS_RATE),
    ),
    routes: None,
};

#[cfg(feature = "teams")]
const TEAMS: PrivilegedShape = PrivilegedShape {
    owner: CoreId::Teams,
    base: BaseRef::Path(BasePath::Api(ApiBase::Teams)),
    group: RouteGroup::Messaging,
    allowance: Allowance::HandlerVerified(HandlerVerifier::TeamsBotToken),
    shape: shape(
        AuthzPolicy::handler_verified(HandlerVerifier::TeamsBotToken),
        ContextKind::HandlerResolved,
        limits(AGENTS_RATE),
    ),
    routes: None,
};

macro_rules! privileged_shapes {
    ($($extra:expr),*) => {
        &[
            ALWAYS[0], ALWAYS[1], ALWAYS[2], ALWAYS[3], ALWAYS[4], ALWAYS[5], ALWAYS[6],
            ALWAYS[7], ALWAYS[8], ALWAYS[9], ALWAYS[10], ALWAYS[11], ALWAYS[12], ALWAYS[13],
            ALWAYS[14], ALWAYS[15], ALWAYS[16], ALWAYS[17], ALWAYS[18] $(, $extra)*
        ]
    };
}

#[cfg(all(feature = "slack", feature = "teams"))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!(SLACK, TEAMS);
#[cfg(all(feature = "slack", not(feature = "teams")))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!(SLACK);
#[cfg(all(not(feature = "slack"), feature = "teams"))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!(TEAMS);
#[cfg(not(any(feature = "slack", feature = "teams")))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!();
