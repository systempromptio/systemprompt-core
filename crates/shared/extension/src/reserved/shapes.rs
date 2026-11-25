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

pub(super) const fn limits(rate: RateLimit) -> RouteLimits {
    RouteLimits {
        rate,
        body: BodyLimit::Default,
    }
}

pub(super) const fn shape(
    policy: AuthzPolicy,
    context: ContextKind,
    limits: RouteLimits,
) -> RouteShape {
    RouteShape {
        policy,
        context,
        limits,
    }
}

const CONTENT_LIMITS: RouteLimits = limits(RateLimit::new("content", RateBudget::Content));
const NOTIFICATIONS_LIMITS: RouteLimits =
    limits(RateLimit::new("notifications", RateBudget::Notifications));
pub(super) const AGENTS_RATE: RateLimit = RateLimit::new("agents", RateBudget::Agents);

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
const CONTENT_ADMIN: RouteShape = shape(AuthzPolicy::admin(), ContextKind::User, CONTENT_LIMITS);

pub const CONTENT_GENERATE_ROUTES: &[(Method, &str)] = &[(Method::POST, "/links/generate")];
pub const CONTENT_LINK_ANALYTICS_ROUTES: &[(Method, &str)] = &[
    (Method::GET, "/links"),
    (Method::GET, "/links/{link_id}/performance"),
    (Method::GET, "/links/{link_id}/clicks"),
    (Method::GET, "/links/campaigns/{campaign_id}/performance"),
    (Method::GET, "/links/journey"),
];
pub const CONTENT_REDIRECT_ROUTES: &[(Method, &str)] = &[(Method::GET, "/r/{short_code}")];
pub const MCP_REGISTRY_ROUTES: &[(Method, &str)] = &[(Method::GET, "/")];
pub const AGENT_REGISTRY_ROUTES: &[(Method, &str)] = &[(Method::GET, "/")];

const BRAIN_LIMITS: RouteLimits = limits(RateLimit::new("brain", RateBudget::Brain));

const ALWAYS: [PrivilegedShape; 20] = [
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
        base: BaseRef::Path(BasePath::Api(ApiBase::Content)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::ReservedPrefix,
        shape: CONTENT_ADMIN,
        routes: Some(CONTENT_LINK_ANALYTICS_ROUTES),
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
        shape: CONTENT_ADMIN,
        routes: None,
    },
    PrivilegedShape {
        owner: CoreId::AnalyticsHttp,
        base: BaseRef::Path(BasePath::Api(ApiBase::Analytics)),
        group: RouteGroup::ContentAndMisc,
        allowance: Allowance::KernelHandle(HandleId::AnalyticsBroadcaster),
        shape: CONTENT_ADMIN,
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

use super::messaging_shapes::{slack, teams};

const SHAPE_COUNT: usize = ALWAYS.len() + slack::SHAPES.len() + teams::SHAPES.len();

const fn joined() -> [PrivilegedShape; SHAPE_COUNT] {
    let mut out = [ALWAYS[0]; SHAPE_COUNT];
    let mut at = 0;
    let parts: [&[PrivilegedShape]; 3] = [&ALWAYS, slack::SHAPES, teams::SHAPES];
    let mut part = 0;
    while part < parts.len() {
        let mut i = 0;
        while i < parts[part].len() {
            out[at] = parts[part][i];
            at += 1;
            i += 1;
        }
        part += 1;
    }
    out
}

const JOINED: [PrivilegedShape; SHAPE_COUNT] = joined();

pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = &JOINED;
