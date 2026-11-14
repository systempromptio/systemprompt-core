//! The closed table of core route declarations.
//!
//! [`CoreDeclaration`] lists every core route group that is treated
//! differently from an install extension's, each by its declarer, exact base
//! path, route group and the one [`Allowance`] it holds (`Plain` for none).
//! It decides two things.
//!
//! Admission: a declaration is refused at boot when it declares something
//! only core may declare: a handler-verified policy (or the handler-resolved
//! context), a base path under one of the reserved prefixes
//! ([`RESERVED_PATHS`]), or a group-level body limit above
//! [`DEFAULT_BODY_LIMIT_BYTES`]. [`admit_extension`] admits one only when a
//! row grants that exact declaration that allowance, matching the verifier
//! and handler-resolved context, the admin policy and user context for a
//! reserved prefix, or the exact byte count for a body limit. A declaration
//! not in the table meets the same typed [`MountError`] as before.
//!
//! Role exposure: [`extension_served`] mounts a listed core declaration on
//! every role that serves its own route group. Every other extension
//! declaration is mounted only where the role serves both
//! `RouteGroup::Extensions` and its own group, so an install extension is
//! served on exactly the roles it always was, whatever group it declares.
//!
//! An extension declaration is matched by its registered extension id, its
//! base path and its route group. The table is a guard-rail against an
//! install extension mounting an ungated or reserved route, or widening its
//! role exposure, by declaration, whether by mistake or by copying a core
//! declaration. It is not a boundary against code linked into the same
//! binary: such code runs with the server's own privileges whatever it
//! declares. Extension ids are unique per process
//! (`LoaderError::DuplicateExtension`), so while a core capability is linked
//! no other extension can hold its id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, HandlerVerifier, RESERVED_PATHS, RouteGroup,
};
use systemprompt_models::modules::ApiPaths;

use super::{DEFAULT_BODY_LIMIT_BYTES, MountError};

/// The component that declares a core route group listed in
/// [`CoreDeclaration`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreDeclarer {
    Server,
    Extension(&'static str),
}

/// The one core-only protection a listed declaration is granted, or `Plain`
/// for a core declaration that holds none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowance {
    Plain,
    HandlerVerified(HandlerVerifier),
    ReservedPrefix,
    BodyLimit(usize),
}

/// A core route group, named by its declarer, base path and route group,
/// with the allowance it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreDeclaration {
    AgentProxy,
    AgentRuns,
    Slack,
    Teams,
    Notifications,
    NotificationsAdmin,
}

impl CoreDeclaration {
    pub const ALL: &'static [Self] = &[
        Self::AgentProxy,
        Self::AgentRuns,
        Self::Slack,
        Self::Teams,
        Self::Notifications,
        Self::NotificationsAdmin,
    ];

    #[must_use]
    pub const fn declarer(self) -> CoreDeclarer {
        match self {
            Self::AgentProxy => CoreDeclarer::Server,
            Self::Slack => CoreDeclarer::Extension("slack"),
            Self::AgentRuns => CoreDeclarer::Extension("agent_runs_api"),
            Self::Teams => CoreDeclarer::Extension("teams"),
            Self::Notifications | Self::NotificationsAdmin => {
                CoreDeclarer::Extension("notifications-http")
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
        }
    }

    #[must_use]
    pub const fn group(self) -> RouteGroup {
        match self {
            Self::AgentProxy | Self::AgentRuns => RouteGroup::Agent,
            Self::Slack | Self::Teams => RouteGroup::Messaging,
            Self::Notifications | Self::NotificationsAdmin => RouteGroup::ContentAndMisc,
        }
    }

    #[must_use]
    pub const fn allowance(self) -> Allowance {
        match self {
            Self::AgentProxy => {
                Allowance::BodyLimit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
            },
            Self::Slack => Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
            Self::Teams => Allowance::HandlerVerified(HandlerVerifier::TeamsBotToken),
            Self::NotificationsAdmin => Allowance::ReservedPrefix,
            Self::AgentRuns | Self::Notifications => Allowance::Plain,
        }
    }

    fn names(self, declarer: CoreDeclarer, declared: &ExtensionRouter) -> bool {
        self.declarer() == declarer
            && self.base_path() == declared.base_path
            && self.group() == declared.group
    }

    fn grants(self, declarer: CoreDeclarer, declared: &ExtensionRouter, wanted: Allowance) -> bool {
        self.names(declarer, declared)
            && self.allowance() == wanted
            && shape_matches(wanted, declared)
    }

    fn granted(declarer: CoreDeclarer, declared: &ExtensionRouter, wanted: Allowance) -> bool {
        Self::ALL.iter().any(|entry| entry.grants(declarer, declared, wanted))
    }

    pub(super) fn admits_body_limit(base_path: &str, declared: usize) -> bool {
        declared <= DEFAULT_BODY_LIMIT_BYTES
            || Self::ALL.iter().any(|entry| {
                entry.base_path() == base_path
                    && entry.allowance() == Allowance::BodyLimit(declared)
            })
    }
}

fn shape_matches(wanted: Allowance, declared: &ExtensionRouter) -> bool {
    let protection = &declared.protection;
    match wanted {
        Allowance::HandlerVerified(verifier) => {
            protection.policy.handler_verifier() == Some(verifier)
                && protection.context == ContextKind::HandlerResolved
        },
        Allowance::ReservedPrefix => {
            protection.policy.handler_verifier().is_none()
                && protection.policy.allowed() == AuthzPolicy::admin().allowed()
                && protection.context == ContextKind::User
        },
        Allowance::BodyLimit(bytes) => protection.body_limit == Some(bytes),
        Allowance::Plain => true,
    }
}

fn reserved(base_path: &str) -> bool {
    RESERVED_PATHS.iter().any(|prefix| base_path.starts_with(prefix))
}

pub fn admit_extension(
    extension: &'static str,
    declared: &ExtensionRouter,
) -> Result<(), MountError> {
    let declarer = CoreDeclarer::Extension(extension);
    let base_path = declared.base_path;
    let protection = &declared.protection;

    if let Some(verifier) = protection.policy.handler_verifier() {
        if !CoreDeclaration::granted(declarer, declared, Allowance::HandlerVerified(verifier)) {
            return Err(MountError::ExtensionHandlerVerified { base_path });
        }
    } else if protection.context == ContextKind::HandlerResolved {
        return Err(MountError::ExtensionHandlerVerified { base_path });
    }

    let reserved_granted = CoreDeclaration::granted(declarer, declared, Allowance::ReservedPrefix);
    if reserved(base_path) && !reserved_granted {
        return Err(MountError::ExtensionReservedPath {
            extension,
            base_path,
        });
    }

    match protection.body_limit {
        Some(bytes)
            if bytes > DEFAULT_BODY_LIMIT_BYTES
                && !CoreDeclaration::granted(declarer, declared, Allowance::BodyLimit(bytes)) =>
        {
            Err(MountError::ExtensionBodyLimitTooLarge {
                base_path,
                declared: bytes,
                ceiling: DEFAULT_BODY_LIMIT_BYTES,
            })
        },
        _ => Ok(()),
    }
}

pub fn extension_served(
    extension: &'static str,
    declared: &ExtensionRouter,
    serves: &dyn Fn(RouteGroup) -> bool,
) -> bool {
    let declarer = CoreDeclarer::Extension(extension);
    let core = CoreDeclaration::ALL.iter().any(|entry| entry.names(declarer, declared));
    serves(declared.group) && (core || serves(RouteGroup::Extensions))
}
