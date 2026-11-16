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
//! 1. a [`PRIVILEGED_SHAPES`] / [`RESERVED_CLI_PATHS`] entry matches it exactly
//!    (base, group, allowance, shape, routes);
//! 2. its owner's extension id equals that entry's [`CoreId::id`];
//! 3. no extension id holds privileged entries under two different [`CoreId`]s.
//!
//! The Slack and Teams shapes exist only in a build with the `slack` or
//! `teams` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::Method;

use crate::cli::KernelGrant;
use crate::protection::RouteGroup;
use crate::router::{Allowance, BaseRef, RouteShape};

mod shapes;

pub use shapes::{CONTENT_GENERATE_ROUTES, CONTENT_REDIRECT_ROUTES, PRIVILEGED_SHAPES};

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
        grants: &[
            KernelGrant::CloudOperator,
            KernelGrant::BootProfile,
            KernelGrant::ConfigReconcile,
        ],
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
