//! Core declarations outside `ExtensionRouter`: gateway route registrations,
//! and which roles build an extension's routers at all.
//!
//! A `register_gateway_route!` registration is served under the gateway
//! credential on every role that serves `RouteGroup::Gateway`, including the
//! gateway role, which serves no extension router. It is therefore a core
//! declaration: [`admit_gateway_route`] admits it only when a
//! [`CoreDeclaration`] row in the `Gateway` group names its owning extension
//! id and its exact path, and the row's whole protection shape equals the
//! one the gateway applies to it (handler-verified by the gateway
//! credential, the registration's scope and budget, the bearer window). Any
//! other registration is refused at boot. [`refuse_overlap`] refuses two
//! registrations whose paths share a segment-bounded prefix, and the mount
//! serves a registered router only for request paths under its own path, so
//! a registration cannot answer for a path its row does not name.
//!
//! [`extension_built`] decides on which roles an extension's routers are
//! built and admitted: a role that serves `RouteGroup::Extensions`, or one
//! that serves the route group of a router row listing that extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteGroup, RouteProtection,
};

use super::MountError;
use super::core_declaration::{CoreDeclaration, CoreDeclarer};

#[must_use]
pub const fn gateway_protection(scope: &'static str, budget: RateBudget) -> RouteProtection {
    RouteProtection::new(
        AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential),
        ContextKind::HandlerResolved,
    )
    .with_rate_limit(RateLimit::new(scope, budget).with_bearer_window())
}

#[must_use]
pub fn path_under(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

pub fn admit_gateway_route(
    extension: &'static str,
    path: &'static str,
    scope: &'static str,
    budget: RateBudget,
) -> Result<(), MountError> {
    let owner = CoreDeclarer::Extension(extension);
    let protection = gateway_protection(scope, budget);
    let row = CoreDeclaration::ALL
        .iter()
        .find(|entry| entry.names(owner, path, RouteGroup::Gateway))
        .ok_or(MountError::UnlistedGatewayRoute { extension, path })?;
    match row.mismatch(&protection) {
        Some(field) => Err(MountError::CoreDeclarationMismatch {
            extension,
            base_path: path,
            field,
        }),
        None => Ok(()),
    }
}

pub fn refuse_overlap(paths: &[&'static str]) -> Result<(), MountError> {
    for (index, path) in paths.iter().enumerate() {
        if let Some(other) = paths[index + 1..]
            .iter()
            .find(|other| path_under(path, other) || path_under(other, path))
        {
            return Err(MountError::GatewayRouteOverlap {
                path: *path,
                other: *other,
            });
        }
    }
    Ok(())
}

pub fn extension_built(extension: &str, serves: &dyn Fn(RouteGroup) -> bool) -> bool {
    serves(RouteGroup::Extensions)
        || CoreDeclaration::ALL.iter().any(|entry| {
            entry.group() != RouteGroup::Gateway
                && entry.declarer() == CoreDeclarer::Extension(extension)
                && serves(entry.group())
        })
}
