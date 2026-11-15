//! The kernel's re-check of an admission handoff.
//!
//! Every admitted route set and gateway route that uses a privilege — a
//! reserved prefix, a closed core base, the engagement web path, a
//! handler-verified policy, the agent body limit, a kernel handle or a
//! gateway path — must match a [`PRIVILEGED_SHAPES`] entry exactly (base,
//! group, allowance, shape, routes); its owner must be that entry's
//! [`CoreId`]; and no owner may hold privileges under two core ids. A
//! handoff built any other way than by the composition therefore gains
//! nothing beyond a listed shape. Reserved prefixes, built-in gateway
//! prefixes and route conflicts are refused here too, whether or not any
//! member claims them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use systemprompt_extension::Admitted;
use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
use systemprompt_extension::reserved::{
    BUILTIN_GATEWAY_PREFIXES, CoreId, PRIVILEGED_SHAPES, PrivilegedShape,
};
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, RouteDef, RouteSet, WebBase,
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
        .find(|prefix| shadows(relative, prefix))
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

fn shadows(relative: &str, prefix: &str) -> bool {
    if prefix == "/" {
        return relative == "/";
    }
    relative == prefix
        || relative
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub fn path_base(set: &RouteSet) -> Result<BasePath, MountError> {
    match set.base() {
        BaseRef::Path(base) => Ok(base),
        base @ BaseRef::Gateway(_) => Err(MountError::RouteSetKind(base)),
    }
}

pub fn full_path(base: BasePath, path: &str) -> String {
    match (base.path(), path) {
        ("/", path) => path.to_owned(),
        (base, "/") => base.to_owned(),
        (base, path) => format!("{base}{path}"),
    }
}

fn refuse_route_conflicts(routes: &[AdmittedRouteSet]) -> Result<(), MountError> {
    let mut seen: HashSet<(http::Method, String)> = HashSet::new();
    for admitted in routes {
        for def in admitted.set.defs() {
            let key = (
                def.method.clone(),
                full_path(path_base(&admitted.set)?, def.path.as_str()),
            );
            if !seen.insert(key.clone()) {
                return Err(MountError::RouteConflict {
                    method: key.0,
                    path: key.1,
                });
            }
        }
    }
    Ok(())
}
