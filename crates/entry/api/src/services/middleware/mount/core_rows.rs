//! The values of each [`CoreDeclaration`] row: owner, base path, route group,
//! allowance, the protection shape it pins and the routes it pins.
//!
//! The analytics and content rows name the routes the `analytics-http` and
//! `content-http` extensions declare: the analytics ingestion and stream
//! routes; engagement tracking, nested at `/track/engagement` under the
//! `WebPath` allowance exactly as the server mounted it; the public and
//! signed-in content routes, two shapes at `/api/v1/content` whose routes
//! interleave under `/links` (`/links`, `/links/journey` and
//! `/links/{link_id}/…` are public, `/links/generate` is signed-in), so the
//! signed-in row pins `POST /links/generate` and the public row's router
//! answers 404 there; and the short-link redirect, merged at the web root as
//! the server mounted it and pinned to `GET /r/…`.
//!
//! Only the `Analytics` row is handed a kernel handle: the analytics event
//! broadcaster its live stream subscribes to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteGroup,
};
use systemprompt_models::modules::ApiPaths;

use super::core_declaration::{Allowance, CoreDeclaration, CoreDeclarer};
use super::core_handles::KernelHandle;
use super::core_pins::{PinnedMethod, RoutePin};

const CONTENT_RATE: RateLimit = RateLimit::new("content", RateBudget::Content);

impl CoreDeclaration {
    #[must_use]
    pub const fn declarer(self) -> CoreDeclarer {
        match self {
            Self::AgentProxy => CoreDeclarer::Server,
            Self::Slack => CoreDeclarer::Extension("slack"),
            Self::AgentRuns => CoreDeclarer::Extension("agent_runs_api"),
            Self::Teams => CoreDeclarer::Extension("teams"),
            Self::Notifications | Self::NotificationsAdmin | Self::BridgeNotifications => {
                CoreDeclarer::Extension("notifications-http")
            },
            Self::Analytics | Self::Engagement => CoreDeclarer::Extension("analytics-http"),
            Self::ContentPublic | Self::ContentAuthenticated | Self::ContentRedirect => {
                CoreDeclarer::Extension("content-http")
            },
        }
    }

    #[must_use]
    pub const fn base_path(self) -> &'static str {
        match self {
            Self::AgentProxy => ApiPaths::AGENTS_BASE,
            Self::Slack => ApiPaths::SLACK_BASE,
            Self::AgentRuns => ApiPaths::AGENT_RUNS,
            Self::Teams => ApiPaths::TEAMS_BASE,
            Self::Notifications => ApiPaths::NOTIFICATIONS,
            Self::NotificationsAdmin => ApiPaths::ADMIN_NOTIFICATIONS,
            Self::BridgeNotifications => ApiPaths::BRIDGE_NOTIFICATIONS,
            Self::Analytics => ApiPaths::ANALYTICS_BASE,
            Self::ContentPublic | Self::ContentAuthenticated => ApiPaths::CONTENT_BASE,
            Self::Engagement => ApiPaths::TRACK_ENGAGEMENT,
            Self::ContentRedirect => WEB_ROOT_BASE_PATH,
        }
    }

    #[must_use]
    pub const fn group(self) -> RouteGroup {
        match self {
            Self::AgentProxy | Self::AgentRuns => RouteGroup::Agent,
            Self::Slack | Self::Teams => RouteGroup::Messaging,
            Self::Notifications
            | Self::NotificationsAdmin
            | Self::Analytics
            | Self::Engagement
            | Self::ContentPublic
            | Self::ContentAuthenticated
            | Self::ContentRedirect => RouteGroup::ContentAndMisc,
            Self::BridgeNotifications => RouteGroup::Gateway,
        }
    }

    #[must_use]
    pub const fn allowance(self) -> Allowance {
        match self {
            Self::AgentProxy => Allowance::BodyLimit,
            Self::Slack => Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            Self::Teams => Allowance::HandlerVerified(HandlerVerifier::TeamsBotToken),
            Self::NotificationsAdmin
            | Self::Analytics
            | Self::ContentPublic
            | Self::ContentAuthenticated => Allowance::ReservedPrefix,
            Self::BridgeNotifications => {
                Allowance::HandlerVerified(HandlerVerifier::GatewayCredential)
            },
            Self::Engagement => Allowance::WebPath,
            Self::AgentRuns | Self::Notifications | Self::ContentRedirect => Allowance::Plain,
        }
    }

    #[must_use]
    pub const fn policy(self) -> AuthzPolicy {
        match self {
            Self::AgentProxy => AuthzPolicy::authenticated(),
            Self::AgentRuns | Self::Notifications | Self::ContentAuthenticated => {
                AuthzPolicy::user()
            },
            Self::Slack => AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
            Self::Teams => AuthzPolicy::handler_verified(HandlerVerifier::TeamsBotToken),
            Self::NotificationsAdmin | Self::Analytics => AuthzPolicy::admin(),
            Self::BridgeNotifications => {
                AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential)
            },
            Self::Engagement | Self::ContentPublic | Self::ContentRedirect => AuthzPolicy::public(),
        }
    }

    #[must_use]
    pub const fn context(self) -> ContextKind {
        match self {
            Self::AgentProxy => ContextKind::A2a,
            Self::AgentRuns
            | Self::Notifications
            | Self::NotificationsAdmin
            | Self::Analytics
            | Self::ContentAuthenticated => ContextKind::User,
            Self::Slack | Self::Teams | Self::BridgeNotifications => ContextKind::HandlerResolved,
            Self::Engagement | Self::ContentPublic | Self::ContentRedirect => ContextKind::Public,
        }
    }

    #[must_use]
    pub const fn rate_limit(self) -> RateLimit {
        match self {
            Self::AgentProxy | Self::Slack | Self::Teams => {
                RateLimit::new("agents", RateBudget::Agents)
            },
            Self::AgentRuns => RateLimit::new("agent_runs", RateBudget::Tasks),
            Self::Notifications | Self::NotificationsAdmin => {
                RateLimit::new("notifications", RateBudget::Notifications)
            },
            Self::BridgeNotifications => {
                RateLimit::new("bridge_notifications", RateBudget::Notifications)
                    .with_bearer_window()
            },
            Self::Analytics
            | Self::Engagement
            | Self::ContentPublic
            | Self::ContentAuthenticated
            | Self::ContentRedirect => CONTENT_RATE,
        }
    }

    #[must_use]
    pub const fn body_limit(self) -> Option<usize> {
        match self {
            Self::AgentProxy => Some(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES),
            Self::AgentRuns
            | Self::Slack
            | Self::Teams
            | Self::Notifications
            | Self::NotificationsAdmin
            | Self::BridgeNotifications
            | Self::Analytics
            | Self::Engagement
            | Self::ContentPublic
            | Self::ContentAuthenticated
            | Self::ContentRedirect => None,
        }
    }

    #[must_use]
    pub const fn route_pins(self) -> &'static [RoutePin] {
        match self {
            Self::ContentAuthenticated => CONTENT_AUTHENTICATED_PINS,
            Self::ContentRedirect => CONTENT_REDIRECT_PINS,
            Self::AgentProxy
            | Self::AgentRuns
            | Self::Slack
            | Self::Teams
            | Self::Notifications
            | Self::NotificationsAdmin
            | Self::BridgeNotifications
            | Self::Analytics
            | Self::Engagement
            | Self::ContentPublic => &[],
        }
    }
}

impl CoreDeclaration {
    #[must_use]
    pub const fn kernel_handles(self) -> &'static [KernelHandle] {
        match self {
            Self::Analytics => &[KernelHandle::AnalyticsBroadcaster],
            Self::AgentProxy
            | Self::AgentRuns
            | Self::Slack
            | Self::Teams
            | Self::Notifications
            | Self::NotificationsAdmin
            | Self::BridgeNotifications
            | Self::Engagement
            | Self::ContentPublic
            | Self::ContentAuthenticated
            | Self::ContentRedirect => &[],
        }
    }
}

const CONTENT_AUTHENTICATED_PINS: &[RoutePin] =
    &[RoutePin::new(PinnedMethod::Post, "/links/generate")];

const CONTENT_REDIRECT_PINS: &[RoutePin] = &[RoutePin::new(PinnedMethod::Get, "/r")];
