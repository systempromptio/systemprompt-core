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
//! root, with no other allowance. Every def's full path — a parameter
//! segment, a catch-all and a service prefix with everything below it
//! included — reaches no reserved path its holder does not own
//! (`refused_reservation`), so neither a plain def under a kernel namespace
//! nor a service covering one passes. Reserved paths, built-in gateway
//! prefixes and route conflicts — a service prefix covering any other route
//! included — are refused here too, whether or not any member claims them.
//! A gateway route's every def lies within the gateway path it claims and
//! under no built-in gateway prefix, and gateway defs join the route-conflict
//! check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
use systemprompt_extension::reserved::{
    BUILTIN_GATEWAY_PREFIXES, CoreId, PRIVILEGED_SHAPES, PrivilegedShape,
};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayDefRefusal, RouteDef, RouteSet,
    WebBase, first_overlap, path_covers, refused_gateway_def, refused_reservation, reserved_below,
};
use systemprompt_extension::{Admitted, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;

use super::MountError;

pub fn recheck(admitted: &Admitted) -> Result<(), MountError> {
    let mut holders: HashMap<ExtensionId, CoreId> = HashMap::new();
    for set in admitted.routes() {
        recheck_route_set(set, &mut holders)?;
        recheck_reserved(set)?;
    }
    for route in admitted.gateway_routes() {
        recheck_gateway_route(route, &mut holders)?;
    }
    refuse_route_conflicts(admitted.routes(), admitted.gateway_routes())
}

fn recheck_route_set(
    admitted: &AdmittedRouteSet,
    holders: &mut HashMap<ExtensionId, CoreId>,
) -> Result<(), MountError> {
    let set = &admitted.set;
    let base = path_base(set)?;
    let reserved = base.reserved_prefix();
    let path_ref = BaseRef::Path(base);
    if let Some(prefix) = reserved.filter(|_| !listed_at(path_ref)) {
        return Err(MountError::ReservedPrefix(prefix));
    }
    if let BasePath::Api(ApiBase::Extension(path)) = base
        && let Some(below) = reserved_below(path)
    {
        return Err(MountError::ReservedPrefix(below.path));
    }
    base.validate()
        .map_err(|source| MountError::InvalidBase { base, source })?;
    let mut needed: Vec<Allowance> = Vec::new();
    match base {
        BasePath::Web(WebBase::TrackEngagement) => needed.push(Allowance::WebPath),
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(WebBase::Root) => {},
        BasePath::Api(_) if reserved.is_none() => needed.push(Allowance::Plain),
        BasePath::Api(_) => {},
    }
    if reserved.is_some() {
        needed.push(Allowance::ReservedPrefix);
    }
    for def in set.defs() {
        if let Some(pinned) = pinned_at(path_ref, def) {
            needed.push(pinned.allowance);
        }
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
    let services = admitted.set.defs().iter().any(RouteDef::is_service);
    let plain = needed.is_empty() && admitted.handles.is_empty();
    if services && (admitted.standing != StandingTag::Install || !open_base || !plain) {
        return Err(MountError::ServiceNotAdmitted {
            base,
            owner: admitted.owner.clone(),
        });
    }
    Ok(())
}

fn recheck_reserved(admitted: &AdmittedRouteSet) -> Result<(), MountError> {
    let holder = match admitted.standing {
        StandingTag::Core => CoreId::for_id(admitted.owner.as_str()),
        StandingTag::Install => None,
    };
    for def in admitted.set.defs() {
        if let Some(reserved) = refused_reservation(admitted.set.base(), def, holder) {
            return Err(MountError::ReservedPrefix(reserved.path));
        }
    }
    Ok(())
}

fn pinned_at(base: BaseRef, def: &RouteDef) -> Option<&'static PrivilegedShape> {
    PRIVILEGED_SHAPES.iter().find(|entry| {
        entry.base == base
            && entry.routes.is_some_and(|routes| {
                routes
                    .iter()
                    .any(|(method, path)| *method == def.method && *path == def.path.as_str())
            })
    })
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
    for def in route.decl.router.defs() {
        match refused_gateway_def(path, def) {
            Some(GatewayDefRefusal::UnderBuiltin { path, builtin }) => {
                return Err(MountError::GatewayRouteUnderBuiltin { path, builtin });
            },
            Some(GatewayDefRefusal::OutsideClaim { path, claimed }) => {
                return Err(MountError::GatewayRouteOutsideClaim { path, claimed });
            },
            None => {},
        }
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

pub(super) const fn path_base(set: &RouteSet) -> Result<BasePath, MountError> {
    match set.base() {
        BaseRef::Path(base) => Ok(base),
        base @ BaseRef::Gateway(_) => Err(MountError::RouteSetKind(base)),
    }
}

fn refuse_route_conflicts(
    routes: &[AdmittedRouteSet],
    gateway_routes: &[AdmittedGatewayRoute],
) -> Result<(), MountError> {
    for admitted in routes {
        path_base(&admitted.set)?;
    }
    let defs = routes
        .iter()
        .map(|admitted| &admitted.set)
        .chain(gateway_routes.iter().map(|route| &route.decl.router))
        .flat_map(|set| set.defs().iter().map(|def| (set.base(), def)));
    match first_overlap(defs) {
        Some(overlap) => Err(MountError::RouteConflict {
            method: overlap.method,
            path: overlap.path,
        }),
        None => Ok(()),
    }
}
