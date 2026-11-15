//! The kernel's reservations: API prefixes, built-in gateway prefixes, CLI
//! paths and the closed set of privileged route shapes.
//!
//! These are paths and shapes, each privileged one named for the core member
//! ([`CoreId`]) that alone may hold it. They are enforced whether or not any
//! member is composed: the composition admits a member's routes and commands
//! against them, and the API mount and CLI registry re-check every admitted
//! entry that uses a privilege, so an admission handoff built any other way
//! gains nothing beyond a shape listed here.
//!
//! Re-check, run on every admitted route set, gateway route and command that
//! uses a privilege:
//! 1. a [`PRIVILEGED_SHAPES`] / [`RESERVED_CLI_PATHS`] entry matches it
//!    exactly (base, group, allowance, shape, routes);
//! 2. its owner's extension id equals that entry's [`CoreId::id`];
//! 3. no extension id holds privileged entries under two different
//!    [`CoreId`]s.
//!
//! The Slack and Teams shapes exist only in a build with the `slack` or
//! `teams` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::Method;

use crate::authz::{AuthzPolicy, HandlerVerifier};
use crate::cli::KernelGrant;
use crate::contribute::HandleId;
use crate::protection::{ContextKind, RateBudget, RateLimit, RouteGroup};
use crate::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayPath, RouteLimits, RouteShape,
    WebBase,
};

pub const WEB_ROOT_BASE_PATH: &str = "/";

pub const RESERVED_API_PREFIXES: &[&str] = &[
    "/api/v1/oauth",
    "/api/v1/users",
    "/api/v1/agents",
    "/api/v1/mcp",
    "/api/v1/stream",
    "/api/v1/content",
    "/api/v1/files",
    "/api/v1/analytics",
    "/api/v1/scheduler",
    "/api/v1/core",
    "/api/v1/admin",
    "/.well-known",
];

pub const BUILTIN_GATEWAY_PREFIXES: &[&str; 31] = &[
    "/",
    "/models",
    "/messages",
    "/responses",
    "/chat/completions",
    "/me/provider-credentials",
    "/otel",
    "/auth/bridge/pat",
    "/auth/bridge/session",
    "/auth/bridge/session-pat",
    "/auth/bridge/oauth-client",
    "/auth/bridge/capabilities",
    "/bridge/pubkey",
    "/bridge/profile",
    "/bridge/whoami",
    "/bridge/manifest",
    "/bridge/plugins",
    "/bridge/device",
    "/bridge/heartbeat",
    "/bridge/stream",
    "/bridge/latest",
    "/bridge/download",
    "/bridge/drive/manifest",
    "/bridge/drive/changes",
    "/bridge/drive/blob",
    "/bridge/drive/commit",
    "/bridge/drive/presence",
    "/bridge/drive/proposals",
    "/bridge/drive/trash",
    "/bridge/drive/move",
    "/bridge/drive/token",
];

/// The closed set of core members that may hold a privileged shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreId {
    ContentHttp,
    AnalyticsHttp,
    NotificationsHttp,
    AgentRunsApi,
    Slack,
    Teams,
    Cloud,
    Kernel,
}

impl CoreId {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ContentHttp => "content-http",
            Self::AnalyticsHttp => "analytics-http",
            Self::NotificationsHttp => "notifications-http",
            Self::AgentRunsApi => "agent_runs_api",
            Self::Slack => "slack",
            Self::Teams => "teams",
            Self::Cloud => "cloud",
            Self::Kernel => "kernel",
        }
    }
}

/// A CLI path only its core owner may claim, with the grants it may request.
#[derive(Debug, Clone, Copy)]
pub struct ReservedCliPath {
    pub owner: CoreId,
    pub path: &'static [&'static str],
    pub grants: &'static [KernelGrant],
}

pub const RESERVED_CLI_PATHS: &[ReservedCliPath] = &[
    ReservedCliPath {
        owner: CoreId::Kernel,
        path: &["admin", "mcp"],
        grants: &[KernelGrant::ChildHost],
    },
    ReservedCliPath {
        owner: CoreId::AnalyticsHttp,
        path: &["analytics"],
        grants: &[KernelGrant::LocalExport, KernelGrant::SkipValidation],
    },
    ReservedCliPath {
        owner: CoreId::Cloud,
        path: &["cloud"],
        grants: &[KernelGrant::CloudOperator],
    },
];

/// One route shape only its core owner may hold.
#[derive(Debug, Clone, Copy)]
pub struct PrivilegedShape {
    pub owner: CoreId,
    pub base: BaseRef,
    pub group: RouteGroup,
    pub allowance: Allowance,
    pub shape: RouteShape,
    pub routes: Option<&'static [(Method, &'static str)]>,
}

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

const CONTENT_PUBLIC: RouteShape = shape(AuthzPolicy::public(), ContextKind::Public, CONTENT_LIMITS);
const CONTENT_USER: RouteShape = shape(AuthzPolicy::user(), ContextKind::User, CONTENT_LIMITS);
const ANALYTICS_ADMIN: RouteShape = shape(AuthzPolicy::admin(), ContextKind::User, CONTENT_LIMITS);

pub const CONTENT_GENERATE_ROUTES: &[(Method, &str)] = &[(Method::POST, "/links/generate")];
pub const CONTENT_REDIRECT_ROUTES: &[(Method, &str)] = &[(Method::GET, "/r")];

const ALWAYS: [PrivilegedShape; 11] = [
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
        shape: shape(AuthzPolicy::admin(), ContextKind::User, NOTIFICATIONS_LIMITS),
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
            ALWAYS[7], ALWAYS[8], ALWAYS[9], ALWAYS[10] $(, $extra)*
        ]
    };
}

/// Every privileged shape in this build, in the order of the reference
/// composition's conferral table.
#[cfg(all(feature = "slack", feature = "teams"))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!(SLACK, TEAMS);
#[cfg(all(feature = "slack", not(feature = "teams")))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!(SLACK);
#[cfg(all(not(feature = "slack"), feature = "teams"))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!(TEAMS);
#[cfg(not(any(feature = "slack", feature = "teams")))]
pub const PRIVILEGED_SHAPES: &[PrivilegedShape] = privileged_shapes!();
