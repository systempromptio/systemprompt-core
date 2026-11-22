//! The kernel's reservations: reserved paths, built-in gateway prefixes, CLI
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
//! A row nested in another owner's namespace (the kernel's or a member's) is
//! bounded ([`nested_rows_are_bounded`], checked at compile time).
//!
//! The Slack and Teams shapes exist only in a build with the `slack` or
//! `teams` feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::Method;
use systemprompt_models::modules::ApiPaths;

use crate::protection::RouteGroup;
use crate::router::{Allowance, BaseRef, RouteShape};

mod cli;
mod nesting;
mod shapes;

pub use cli::{RESERVED_CLI_PATHS, ReservedCliPath};
pub use nesting::{nested_rows_are_bounded, nested_under_another_owner};
pub use shapes::{
    CONTENT_GENERATE_ROUTES, CONTENT_REDIRECT_ROUTES, MCP_REGISTRY_ROUTES, PRIVILEGED_SHAPES,
};

pub const WEB_ROOT_BASE_PATH: &str = "/";

/// How far a [`ReservedPath`] extends.
///
/// `Prefix` holds every path whose text
/// starts with it (so `/api/v1/contentx` is held with `/api/v1/content`),
/// `Subtree` the path and every path below it segment by segment, `Exact`
/// the path alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReservedReach {
    Prefix,
    Subtree,
    Exact,
}

/// A path only its core owner may route, whether or not that owner is
/// composed.
///
/// A nested entry (`/api/v1/admin/notifications` inside
/// `/api/v1/admin`) is held by its own owner; the enclosing owner keeps the
/// rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservedPath {
    pub owner: CoreId,
    pub path: &'static str,
    pub reach: ReservedReach,
}

const fn kernel(path: &'static str, reach: ReservedReach) -> ReservedPath {
    ReservedPath {
        owner: CoreId::Kernel,
        path,
        reach,
    }
}

const fn held(owner: CoreId, path: &'static str) -> ReservedPath {
    ReservedPath {
        owner,
        path,
        reach: ReservedReach::Prefix,
    }
}

const fn exact(owner: CoreId, path: &'static str) -> ReservedPath {
    ReservedPath {
        owner,
        path,
        reach: ReservedReach::Exact,
    }
}

const fn subtree(owner: CoreId, path: &'static str) -> ReservedPath {
    ReservedPath {
        owner,
        path,
        reach: ReservedReach::Subtree,
    }
}

pub const RESERVED_PATHS: &[ReservedPath] = &[
    kernel(ApiPaths::DISCOVERY, ReservedReach::Exact),
    kernel(ApiPaths::AUTH_ME, ReservedReach::Exact),
    kernel(ApiPaths::META_BASE, ReservedReach::Prefix),
    kernel("/api/v1/oauth", ReservedReach::Prefix),
    kernel("/api/v1/users", ReservedReach::Prefix),
    kernel(ApiPaths::AGENTS_BASE, ReservedReach::Prefix),
    kernel(ApiPaths::MCP_BASE, ReservedReach::Prefix),
    kernel(ApiPaths::STREAM_BASE, ReservedReach::Prefix),
    kernel("/api/v1/files", ReservedReach::Prefix),
    kernel("/api/v1/scheduler", ReservedReach::Prefix),
    kernel(ApiPaths::CORE_BASE, ReservedReach::Prefix),
    kernel(ApiPaths::ADMIN_BASE, ReservedReach::Prefix),
    kernel(ApiPaths::DOCUMENTS, ReservedReach::Prefix),
    kernel(ApiPaths::DOCUMENT_FOLDERS, ReservedReach::Prefix),
    kernel(ApiPaths::UPLOADS, ReservedReach::Prefix),
    kernel("/api/v1/internal", ReservedReach::Prefix),
    kernel("/api/v1/issues", ReservedReach::Prefix),
    kernel(ApiPaths::SYNC_BASE, ReservedReach::Prefix),
    kernel(ApiPaths::WEBHOOK, ReservedReach::Prefix),
    kernel(ApiPaths::HEALTH, ReservedReach::Prefix),
    kernel("/api/v1/consumer", ReservedReach::Prefix),
    kernel(ApiPaths::GATEWAY_PUBLIC_BASE, ReservedReach::Prefix),
    kernel(ApiPaths::GATEWAY_BASE, ReservedReach::Subtree),
    kernel(ApiPaths::WELLKNOWN_BASE, ReservedReach::Prefix),
    kernel(WEB_ROOT_BASE_PATH, ReservedReach::Exact),
    kernel("/health", ReservedReach::Exact),
    kernel(ApiPaths::LIVEZ, ReservedReach::Exact),
    kernel(ApiPaths::READYZ, ReservedReach::Exact),
    kernel("/auth/link-passkey", ReservedReach::Exact),
    held(CoreId::ContentHttp, ApiPaths::CONTENT_BASE),
    subtree(CoreId::ContentHttp, "/r"),
    held(CoreId::AnalyticsHttp, ApiPaths::ANALYTICS_BASE),
    subtree(CoreId::AnalyticsHttp, ApiPaths::TRACK_BASE),
    held(CoreId::NotificationsHttp, ApiPaths::NOTIFICATIONS),
    subtree(CoreId::NotificationsHttp, ApiPaths::ADMIN_NOTIFICATIONS),
    held(CoreId::AgentRunsApi, ApiPaths::AGENT_RUNS),
    held(CoreId::Slack, ApiPaths::SLACK_BASE),
    held(CoreId::Teams, ApiPaths::TEAMS_BASE),
    held(CoreId::Cloud, ApiPaths::CLOUD_TENANTS),
    exact(CoreId::McpHttp, ApiPaths::MCP_REGISTRY),
    held(CoreId::MarketplaceHttp, ApiPaths::MARKETPLACE_BASE),
    held(CoreId::BrainHttp, ApiPaths::BRAIN),
    subtree(CoreId::BrainHttp, ApiPaths::ADMIN_BRAIN),
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
    MarketplaceHttp,
    FilesHttp,
    McpHttp,
    AgentHttp,
    BrainHttp,
    Kernel,
}

impl CoreId {
    pub const ALL: [Self; 13] = [
        Self::ContentHttp,
        Self::AnalyticsHttp,
        Self::NotificationsHttp,
        Self::AgentRunsApi,
        Self::Slack,
        Self::Teams,
        Self::Cloud,
        Self::MarketplaceHttp,
        Self::FilesHttp,
        Self::McpHttp,
        Self::AgentHttp,
        Self::BrainHttp,
        Self::Kernel,
    ];

    #[must_use]
    pub fn for_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|core| core.id() == id)
    }

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
            Self::MarketplaceHttp => "marketplace-http",
            Self::FilesHttp => "files-http",
            Self::McpHttp => "mcp-http",
            Self::AgentHttp => "agent-http",
            Self::BrainHttp => "brain-http",
            Self::Kernel => "kernel",
        }
    }
}

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
