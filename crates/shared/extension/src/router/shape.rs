//! The axum-free route value types: bases, specs, shapes and allowances.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::modules::ApiPaths;

use super::overlap::{reserved_below, reserved_holding};
use crate::authz::{AuthzPolicy, HandlerVerifier};
use crate::contribute::{ContributionError, HandleId};
use crate::protection::{ContextKind, RateLimit};
use crate::reserved::WEB_ROOT_BASE_PATH;

/// Where a route set mounts: a closed set of `ApiPaths` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasePath {
    Api(ApiBase),
    Web(WebBase),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiBase {
    Analytics,
    Content,
    Notifications,
    AdminNotifications,
    AgentRuns,
    Agents,
    Slack,
    Teams,
    Extension(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WebBase {
    TrackEngagement,
    Root,
}

impl BasePath {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Api(api) => match api {
                ApiBase::Analytics => ApiPaths::ANALYTICS_BASE,
                ApiBase::Content => ApiPaths::CONTENT_BASE,
                ApiBase::Notifications => ApiPaths::NOTIFICATIONS,
                ApiBase::AdminNotifications => ApiPaths::ADMIN_NOTIFICATIONS,
                ApiBase::AgentRuns => ApiPaths::AGENT_RUNS,
                ApiBase::Agents => ApiPaths::AGENTS_BASE,
                ApiBase::Slack => ApiPaths::SLACK_BASE,
                ApiBase::Teams => ApiPaths::TEAMS_BASE,
                ApiBase::Extension(path) => path,
            },
            Self::Web(WebBase::TrackEngagement) => ApiPaths::TRACK_ENGAGEMENT,
            Self::Web(WebBase::Root) => WEB_ROOT_BASE_PATH,
        }
    }

    #[must_use]
    pub const fn is_web_root(self) -> bool {
        matches!(self, Self::Web(WebBase::Root))
    }

    #[must_use]
    pub fn reserved_prefix(self) -> Option<&'static str> {
        reserved_prefix_of(self.path())
    }

    pub fn validate(self) -> Result<(), ContributionError> {
        match self {
            Self::Api(ApiBase::Extension(path)) => validate_extension_base(path),
            Self::Api(_) | Self::Web(_) => Ok(()),
        }
    }
}

/// The gateway paths a gateway route may claim; an install holds none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatewayPath {
    BridgeNotifications,
}

impl GatewayPath {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::BridgeNotifications => ApiPaths::BRIDGE_NOTIFICATIONS,
        }
    }
}

/// A privileged shape's base: an API or web base, or a gateway path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseRef {
    Path(BasePath),
    Gateway(GatewayPath),
}

/// What a route set uses beyond a plain, unreserved mount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Allowance {
    Plain,
    ReservedPrefix,
    WebPath,
    BodyLimit,
    HandlerVerified(HandlerVerifier),
    KernelHandle(HandleId),
    GatewayRoute,
}

/// The request body ceiling a route carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyLimit {
    Default,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteLimits {
    pub rate: RateLimit,
    pub body: BodyLimit,
}

/// The policy, context and limits one route is mounted behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteSpec {
    pub policy: AuthzPolicy,
    pub context: ContextKind,
    pub limits: RouteLimits,
}

impl RouteSpec {
    #[must_use]
    pub const fn new(policy: AuthzPolicy, context: ContextKind, limits: RouteLimits) -> Self {
        Self {
            policy,
            context,
            limits,
        }
    }

    #[must_use]
    pub fn typed<Req: schemars::JsonSchema, Res: schemars::JsonSchema>(self) -> TypedSpec {
        TypedSpec {
            spec: self,
            openapi: OperationSpec {
                request: schema_of::<Req>,
                response: schema_of::<Res>,
            },
        }
    }

    #[must_use]
    pub const fn shape(self) -> RouteShape {
        RouteShape {
            policy: self.policy,
            context: self.context,
            limits: self.limits,
        }
    }
}

/// The shape a conferral or privileged-shape entry binds a route to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteShape {
    pub policy: AuthzPolicy,
    pub context: ContextKind,
    pub limits: RouteLimits,
}

/// A route spec with the request and response schemas of its operation.
#[derive(Debug, Clone, Copy)]
pub struct TypedSpec {
    pub spec: RouteSpec,
    pub openapi: OperationSpec,
}

#[derive(Debug, Clone, Copy)]
pub struct OperationSpec {
    pub request: fn() -> schemars::Schema,
    pub response: fn() -> schemars::Schema,
}

fn schema_of<T: schemars::JsonSchema>() -> schemars::Schema {
    schemars::schema_for!(T)
}

/// A route path relative to its base, validated when declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoutePath(&'static str);

impl RoutePath {
    pub fn parse(path: &'static str) -> Result<Self, ContributionError> {
        if route_path_is_valid(path, true) {
            Ok(Self(path))
        } else {
            Err(ContributionError::InvalidPath(path))
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    #[must_use]
    pub fn is_service_prefix(self) -> bool {
        self.0 != "/" && !self.0.contains(['{', '}', '*'])
    }
}

/// One declared route: method, path relative to the base, spec and optional
/// operation schemas.
#[derive(Debug, Clone)]
pub struct RouteDef {
    pub method: RouteMethod,
    pub path: RoutePath,
    pub spec: RouteSpec,
    pub openapi: Option<OperationSpec>,
}

impl RouteDef {
    #[must_use]
    pub const fn is_service(&self) -> bool {
        matches!(self.method, RouteMethod::Any)
    }
}

/// The method a declared route answers.
///
/// [`RouteMethod::Any`] marks a service
/// def, one opaque entry `ANY {path}/*`: it answers every method on every
/// path under its prefix, carries no operation schemas, and is admitted only
/// for an install at an unreserved extension base or the web root.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RouteMethod {
    One(http::Method),
    Any,
}

impl std::fmt::Display for RouteMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One(method) => method.fmt(f),
            Self::Any => f.write_str("ANY"),
        }
    }
}

impl PartialEq<http::Method> for RouteMethod {
    fn eq(&self, other: &http::Method) -> bool {
        matches!(self, Self::One(method) if method == other)
    }
}

impl PartialEq<RouteMethod> for http::Method {
    fn eq(&self, other: &RouteMethod) -> bool {
        other == self
    }
}

#[must_use]
pub fn reserved_prefix_of(path: &str) -> Option<&'static str> {
    reserved_holding(path).map(|entry| entry.path)
}

fn route_path_is_valid(path: &str, allow_params: bool) -> bool {
    if path == "/" {
        return true;
    }
    path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains("//")
        && !path.contains("..")
        && path.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(b, b'-' | b'_' | b'/' | b'.')
                || (allow_params && matches!(b, b'{' | b'}' | b'*'))
        })
}

fn validate_extension_base(path: &'static str) -> Result<(), ContributionError> {
    if !path.starts_with("/api/")
        || !route_path_is_valid(path, false)
        || path.contains('.')
        || reserved_below(path).is_some()
    {
        return Err(ContributionError::InvalidBase(path));
    }
    Ok(())
}
