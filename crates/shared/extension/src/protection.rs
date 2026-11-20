//! Route protection and site-auth configuration value types.
//!
//! A [`RouteProtection`] is everything the host applies around a mounted
//! group: the authorization policy, the context middleware kind, the rate
//! limit and bearer window, the body limit, the closed set of extra
//! protection layers and the frame-options override. The [`RouteGroup`]
//! decides which node roles serve a group.
//!
//! There is no default policy: a protection cannot be built without an
//! [`AuthzPolicy`] and a [`ContextKind`]. The host applies everything it
//! carries through its own enforcement primitives; this module is data only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::auth::UserType;
use systemprompt_provider_contracts::FrameOptions;

use crate::authz::AuthzPolicy;

/// The request-context middleware a route group is mounted behind.
///
/// `HandlerResolved` mounts none and pairs only with a handler-verified
/// policy. `UserUi` is the user context that also admits a person's UI
/// session (the `ui` audience); it is conferred, never plain. Deliberately exhaustive: the host matches every kind, so a new kind
/// cannot reach a wildcard arm and mount with less protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextKind {
    Public,
    User,
    UserUi,
    A2a,
    Mcp,
    HandlerResolved,
}

/// The per-second budget a rate limit draws: a named profile rate or a
/// fixed rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateBudget {
    OauthPublic,
    OauthAuth,
    Contexts,
    Tasks,
    Artifacts,
    AgentRegistry,
    Agents,
    McpRegistry,
    Mcp,
    Stream,
    Content,
    Notifications,
    Brain,
    Gateway,
    BridgeAuth,
    PerSecond(u64),
}

impl RateBudget {
    #[must_use]
    pub const fn per_second(self, config: &RateLimitsConfig) -> u64 {
        match self {
            Self::OauthPublic => config.oauth_public_per_second,
            Self::OauthAuth => config.oauth_auth_per_second,
            Self::Contexts => config.contexts_per_second,
            Self::Tasks => config.tasks_per_second,
            Self::Artifacts => config.artifacts_per_second,
            Self::AgentRegistry => config.agent_registry_per_second,
            Self::Agents => config.agents_per_second,
            Self::McpRegistry => config.mcp_registry_per_second,
            Self::Mcp => config.mcp_per_second,
            Self::Stream => config.stream_per_second,
            Self::Content => config.content_per_second,
            Self::Notifications => config.notifications_per_second,
            Self::Brain => config.brain_per_second,
            Self::Gateway => config.gateway_per_second,
            Self::BridgeAuth => config.bridge_auth_per_second,
            Self::PerSecond(per_second) => per_second,
        }
    }
}

/// A rate limit charged to a named scope, optionally also charged to the
/// replica-shared window by the signature-verified bearer token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub scope: &'static str,
    pub budget: RateBudget,
    pub bearer_window: bool,
}

impl RateLimit {
    #[must_use]
    pub const fn new(scope: &'static str, budget: RateBudget) -> Self {
        Self {
            scope,
            budget,
            bearer_window: false,
        }
    }

    #[must_use]
    pub const fn with_bearer_window(mut self) -> Self {
        self.bearer_window = true;
        self
    }
}

/// The closed set of extra protection layers a route group can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtectionLayer {
    RefuseFileManagedClientWrites,
    SiteAuth,
}

/// A group of routes mounted together; a node's role decides which groups
/// it serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteGroup {
    Oauth,
    Agent,
    McpAndStream,
    ContentAndMisc,
    InternalLinks,
    Messaging,
    Extensions,
    Discovery,
    AuthenticatedDiscovery,
    WellKnown,
    Managed,
    Gateway,
    Static,
    Probes,
}

/// Everything the host applies around a router before mounting it.
#[derive(Debug, Clone)]
pub struct RouteProtection {
    pub policy: AuthzPolicy,
    pub context: ContextKind,
    pub rate_limit: Option<RateLimit>,
    pub body_limit: Option<usize>,
    pub layers: Vec<ProtectionLayer>,
    pub frame_options: Option<FrameOptions>,
}

impl RouteProtection {
    #[must_use]
    pub const fn new(policy: AuthzPolicy, context: ContextKind) -> Self {
        Self {
            policy,
            context,
            rate_limit: None,
            body_limit: None,
            layers: Vec::new(),
            frame_options: None,
        }
    }

    #[must_use]
    pub const fn with_rate_limit(mut self, rate_limit: RateLimit) -> Self {
        self.rate_limit = Some(rate_limit);
        self
    }

    #[must_use]
    pub const fn with_body_limit(mut self, bytes: usize) -> Self {
        self.body_limit = Some(bytes);
        self
    }

    #[must_use]
    pub fn with_layer(mut self, layer: ProtectionLayer) -> Self {
        if !self.layers.contains(&layer) {
            self.layers.push(layer);
        }
        self
    }

    #[must_use]
    pub const fn with_frame_options(mut self, frame_options: FrameOptions) -> Self {
        self.frame_options = Some(frame_options);
        self
    }

    #[must_use]
    pub fn requires_authentication(&self) -> bool {
        !self.policy.permits(UserType::Anon)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SiteAuthConfig {
    pub login_path: &'static str,
    pub protected_prefixes: &'static [&'static str],
    pub public_prefixes: &'static [&'static str],
    pub required_scope: &'static str,
}
