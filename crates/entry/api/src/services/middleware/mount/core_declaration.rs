//! The closed table of core route declarations.
//!
//! [`CoreDeclaration`] lists every core route group that is treated
//! differently from an install extension's, each by its owner, exact base
//! path, route group and the one [`Allowance`] it holds (`Plain` for none).
//! It decides two things.
//!
//! Admission: a declaration is refused at boot when it declares something
//! only core may declare: a handler-verified policy (or the handler-resolved
//! context), a base path under one of the reserved prefixes
//! ([`RESERVED_PATHS`]), or a group-level body limit above
//! [`DEFAULT_BODY_LIMIT_BYTES`]. [`admit_extension`] admits one only when a
//! row with that allowance names the declaration and matches its whole
//! protection shape. A declaration
//! not in the table meets the same typed [`MountError`] as before.
//!
//! Role exposure: [`extension_served`] mounts a listed core declaration on
//! every role that serves its own route group. Every other extension
//! declaration is mounted only where the role serves both
//! `RouteGroup::Extensions` and its own group, so an install extension is
//! served on exactly the roles it always was, whatever group it declares.
//!
//! The rows' values (owner, base path, group, allowance and shape) live in
//! `core_rows`. A row whose owner is not linked, or whose routes the server
//! still mounts itself, is inert: rows are consulted only for extension
//! declarations, by the declaring extension's id.
//!
//! Every row pins the whole protection shape its declaration is admitted
//! with: policy, context kind, rate-limit scope, budget and bearer window,
//! body limit, protection layers and frame options. Drift is judged per
//! owner: an extension id that owns any router row may declare only routers
//! that equal one of its rows in base path, route group and shape (one owner
//! may list several shapes at one base path and group, one row each). Any other
//! declaration from that id is refused at boot with
//! `MountError::CoreDeclarationMismatch` naming the field that differs (the
//! base path or group when no row of that id matches both), so a core
//! declaration that moves or changes its protection fails until the table
//! changes with it. Install ids own no row and are unaffected.
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
    ContextKind, ExtensionRouter, HandlerVerifier, RESERVED_PATHS, RouteGroup, RouteProtection,
};

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
    BodyLimit,
}

/// A core route group, named by its owner, base path and route group,
/// with the allowance it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreDeclaration {
    AgentProxy,
    AgentRuns,
    Slack,
    Teams,
    Notifications,
    NotificationsAdmin,
    BridgeNotifications,
    Analytics,
    Engagement,
    ContentPublic,
    ContentAuthenticated,
    ContentRedirect,
}

impl CoreDeclaration {
    pub const ALL: &'static [Self] = &[
        Self::AgentProxy,
        Self::AgentRuns,
        Self::Slack,
        Self::Teams,
        Self::Notifications,
        Self::NotificationsAdmin,
        Self::BridgeNotifications,
        Self::Analytics,
        Self::Engagement,
        Self::ContentPublic,
        Self::ContentAuthenticated,
        Self::ContentRedirect,
    ];

    pub(super) fn names(self, owner: CoreDeclarer, base_path: &str, group: RouteGroup) -> bool {
        self.declarer() == owner && self.base_path() == base_path && self.group() == group
    }

    fn grants(self, owner: CoreDeclarer, declared: &ExtensionRouter, wanted: Allowance) -> bool {
        self.names(owner, declared.base_path, declared.group)
            && self.allowance() == wanted
            && self.shape_matches(&declared.protection)
    }

    fn granted(owner: CoreDeclarer, declared: &ExtensionRouter, wanted: Allowance) -> bool {
        Self::ALL
            .iter()
            .any(|entry| entry.grants(owner, declared, wanted))
    }

    pub(super) fn admits_body_limit(base_path: &str, protection: &RouteProtection) -> bool {
        protection
            .body_limit
            .is_none_or(|declared| declared <= DEFAULT_BODY_LIMIT_BYTES)
            || Self::ALL.iter().any(|entry| {
                entry.base_path() == base_path
                    && entry.allowance() == Allowance::BodyLimit
                    && entry.shape_matches(protection)
            })
    }
}

fn reserved(base_path: &str) -> bool {
    RESERVED_PATHS
        .iter()
        .any(|prefix| base_path.starts_with(prefix))
}

pub fn admit_extension(
    extension: &'static str,
    declared: &ExtensionRouter,
) -> Result<(), MountError> {
    let owner = CoreDeclarer::Extension(extension);
    let base_path = declared.base_path;
    let protection = &declared.protection;

    if let Some(field) = CoreDeclaration::owner_drift(owner, declared) {
        return Err(MountError::CoreDeclarationMismatch {
            extension,
            base_path,
            field,
        });
    }

    if let Some(verifier) = protection.policy.handler_verifier() {
        if !CoreDeclaration::granted(owner, declared, Allowance::HandlerVerified(verifier)) {
            return Err(MountError::ExtensionHandlerVerified { base_path });
        }
    } else if protection.context == ContextKind::HandlerResolved {
        return Err(MountError::ExtensionHandlerVerified { base_path });
    }

    let reserved_granted = CoreDeclaration::granted(owner, declared, Allowance::ReservedPrefix);
    if reserved(base_path) && !reserved_granted {
        return Err(MountError::ExtensionReservedPath {
            extension,
            base_path,
        });
    }

    match protection.body_limit {
        Some(bytes)
            if bytes > DEFAULT_BODY_LIMIT_BYTES
                && !CoreDeclaration::granted(owner, declared, Allowance::BodyLimit) =>
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
    let owner = CoreDeclarer::Extension(extension);
    let core = CoreDeclaration::ALL.iter().any(|entry| {
        entry.names(owner, declared.base_path, declared.group)
            && entry.shape_matches(&declared.protection)
    });
    serves(declared.group) && (core || serves(RouteGroup::Extensions))
}
