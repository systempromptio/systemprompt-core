//! The kernel's re-check of an admission handoff.
//!
//! Every admitted route set and gateway route that uses a privilege — a
//! reserved prefix, a closed core base, the engagement web path, a
//! handler-verified policy, the agent body limit, a kernel handle or a
//! gateway path — must match a [`PRIVILEGED_SHAPES`] entry exactly (base,
//! group, allowance, shape, routes); its owner must be that entry's
//! [`CoreId`]; and no owner may hold privileges under two core ids. A
//! handoff built any other way than by the composition therefore gains
//! nothing beyond a listed shape. A nested service is never privileged: it
//! is held only by an install, at an unreserved extension base or the web
//! root, with no other allowance, and its full prefix lies under no reserved
//! prefix. Reserved prefixes, built-in gateway prefixes and route conflicts —
//! a service prefix covering any other route included — are refused here
//! too, whether or not any member claims them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_extension::{Admitted, StandingTag};
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
use systemprompt_extension::reserved::{
    BUILTIN_GATEWAY_PREFIXES, CoreId, PRIVILEGED_SHAPES, PrivilegedShape,
};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, RouteDef, RouteSet, WebBase, first_overlap,
    full_path, path_covers, reserved_prefix_of,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;

use super::MountError;

pub fn recheck(admitted: &Admitted) -> Result<(), MountError> {
    let mut holders: HashMap<ExtensionId, CoreId> = HashMap::new();
    for set in admitted.routes() {
        recheck_route_set(set, &mut holders)?;
    }
    for route in admitted.gateway_routes() {
        recheck_gateway_route(route, &mut holders)?;
    }
    refuse_route_conflicts(admitted.routes())
}

fn recheck_route_set(
    admitted: &AdmittedRouteSet,
    holders: &mut HashMap<ExtensionId, CoreId>,
) -> Result<(), MountError> {
    let set = &admitted.set;
    let base = path_base(set)?;
    let reserved = base.reserved_prefix();
    let mut needed: Vec<Allowance> = Vec::new();
    if reserved.is_some() {
        needed.push(Allowance::ReservedPrefix);
    }
    match base {
        BasePath::Web(WebBase::TrackEngagement) => needed.push(Allowance::WebPath),
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(WebBase::Root) => {},
        BasePath::Api(_) if reserved.is_none() => needed.push(Allowance::Plain),
        BasePath::Api(_) => {},
    }
    for def in set.defs() {
        if let Some(verifier) = def.spec.policy.handler_verifier() {
            needed.push(Allowance::HandlerVerified(verifier));
        }
        if def.spec.limits.body == BodyLimit::Agent {
            needed.push(Allowance::BodyLimit);
        }
    }
    recheck_services(admitted, base, &needed)?;
    let privileged = !needed.is_empty() || !admitted.handles.is_empty();
    if !privileged {
        return Ok(());
    }
    let path_ref = BaseRef::Path(base);
    let allowance = needed.first().copied().unwrap_or(Allowance::Plain);
    for def in set.defs() {
        let entry = matching_entry(path_ref, set.group(), def).ok_or(match reserved {
            Some(prefix) if !listed_at(path_ref) => MountError::ReservedPrefix(prefix),
            _ => MountError::UnlistedPrivilege {
                base: path_ref,
                allowance,
            },
        })?;
        check_owner(path_ref, &admitted.owner, entry.owner, holders)?;
    }
    for handle in &admitted.handles {
        let entry = PRIVILEGED_SHAPES
            .iter()
            .find(|entry| {
                entry.base == path_ref
                    && entry.group == set.group()
                    && entry.allowance == Allowance::KernelHandle(*handle)
            })
            .ok_or(MountError::UnlistedPrivilege {
                base: path_ref,
                allowance: Allowance::KernelHandle(*handle),
            })?;
        check_owner(path_ref, &admitted.owner, entry.owner, holders)?;
    }
    Ok(())
}

fn recheck_services(
    admitted: &AdmittedRouteSet,
    base: BasePath,
    needed: &[Allowance],
) -> Result<(), MountError> {
    let open_base = matches!(
        base,
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(WebBase::Root)
    );
    for def in admitted.set.defs().iter().filter(|def| def.is_service()) {
        let path = full_path(BaseRef::Path(base), def.path.as_str());
        if let Some(prefix) = reserved_prefix_of(&path) {
            return Err(MountError::ReservedPrefix(prefix));
        }
        let plain = needed.is_empty() && admitted.handles.is_empty();
        if admitted.standing != StandingTag::Install || !open_base || !plain {
            return Err(MountError::ServiceNotAdmitted {
                base,
                owner: admitted.owner.clone(),
            });
        }
    }
    Ok(())
}

fn listed_at(base: BaseRef) -> bool {
    PRIVILEGED_SHAPES.iter().any(|entry| entry.base == base)
}

fn matching_entry(
    base: BaseRef,
    group: systemprompt_extension::RouteGroup,
    def: &RouteDef,
) -> Option<&'static PrivilegedShape> {
    let at_base: Vec<&'static PrivilegedShape> = PRIVILEGED_SHAPES
        .iter()
        .filter(|entry| {
            entry.base == base && !matches!(entry.allowance, Allowance::KernelHandle(_))
        })
        .collect();
    let listed = |entry: &PrivilegedShape| {
        entry.routes.is_some_and(|routes| {
            routes
                .iter()
                .any(|(method, path)| *method == def.method && *path == def.path.as_str())
        })
    };
    let pinned = at_base.iter().any(|entry| listed(entry));
    let mut matches = at_base.into_iter().filter(|entry| {
        entry.group == group
            && entry.shape == def.spec.shape()
            && match entry.routes {
                Some(_) => listed(entry),
                None => !pinned,
            }
    });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn check_owner(
    base: BaseRef,
    owner: &ExtensionId,
    expected: CoreId,
    holders: &mut HashMap<ExtensionId, CoreId>,
) -> Result<(), MountError> {
    let mismatch = || MountError::PrivilegeOwnerMismatch {
        base,
        owner: owner.clone(),
        expected,
    };
    if owner.as_str() != expected.id() {
        return Err(mismatch());
    }
    match holders.insert(owner.clone(), expected) {
        Some(previous) if previous != expected => Err(mismatch()),
        _ => Ok(()),
    }
}

fn recheck_gateway_route(
    route: &AdmittedGatewayRoute,
    holders: &mut HashMap<ExtensionId, CoreId>,
) -> Result<(), MountError> {
    let Some(path) = route.decl.path() else {
        return Err(MountError::RouteSetKind(route.decl.router.base()));
    };
    let base = BaseRef::Gateway(path);
    let relative = ApiPaths::gateway_relative(path.path());
    if let Some(builtin) = BUILTIN_GATEWAY_PREFIXES
        .iter()
        .copied()
        .find(|prefix| path_covers(relative, prefix))
    {
        return Err(MountError::GatewayShadowsBuiltin(builtin));
    }
    let entry = PRIVILEGED_SHAPES
        .iter()
        .find(|entry| {
            entry.base == base
                && entry.allowance == Allowance::GatewayRoute
                && entry.group == route.decl.router.group()
                && entry.shape.limits == route.decl.limits
                && route
                    .decl
                    .router
                    .defs()
                    .iter()
                    .all(|def| def.spec.shape() == entry.shape)
        })
        .ok_or(MountError::UnlistedPrivilege {
            base,
            allowance: Allowance::GatewayRoute,
        })?;
    check_owner(base, &route.owner, entry.owner, holders)
}

pub(super) fn path_base(set: &RouteSet) -> Result<BasePath, MountError> {
    match set.base() {
        BaseRef::Path(base) => Ok(base),
        base @ BaseRef::Gateway(_) => Err(MountError::RouteSetKind(base)),
    }
}

fn refuse_route_conflicts(routes: &[AdmittedRouteSet]) -> Result<(), MountError> {
    for admitted in routes {
        path_base(&admitted.set)?;
    }
    let defs = routes
        .iter()
        .flat_map(|admitted| admitted.set.defs().iter().map(|def| (admitted.set.base(), def)));
    match first_overlap(defs) {
        Some(overlap) => Err(MountError::RouteConflict {
            method: overlap.method,
            path: overlap.path,
        }),
        None => Ok(()),
    }
}
