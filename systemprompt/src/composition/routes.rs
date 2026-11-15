//! Route-set and gateway-route admission against the conferrals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(feature = "api")]

use std::collections::HashSet;

use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
use systemprompt_extension::router::{Allowance, BasePath, BaseRef, RouteSet};

use super::error::CompositionError;
use super::{Conferral, DeclaredMember};

pub(super) fn admit_route_set(
    member: &DeclaredMember,
    set: RouteSet,
) -> Result<AdmittedRouteSet, CompositionError> {
    let base = set.base();
    let conferrals: Vec<&'static Conferral> = member
        .standing
        .terms()
        .map(|terms| {
            terms
                .conferrals()
                .filter(|c| c.base == BaseRef::Path(base))
                .collect()
        })
        .unwrap_or_default();
    if conferrals.is_empty() {
        admit_unconferred(member, &set)?;
        return Ok(AdmittedRouteSet {
            owner: member.id.clone(),
            standing: member.standing.tag(),
            set,
            handles: Vec::new(),
        });
    }
    let counting: Vec<&Conferral> = conferrals
        .iter()
        .copied()
        .filter(|c| !matches!(c.allowance, Allowance::KernelHandle(_)))
        .collect();
    for def in set.defs() {
        let pinned = counting.iter().any(|c| {
            c.routes.is_some_and(|routes| {
                routes
                    .iter()
                    .any(|(m, p)| *m == def.method && *p == def.path.as_str())
            })
        });
        let matches = counting
            .iter()
            .filter(|c| {
                c.group == set.group()
                    && c.shape == def.spec.shape()
                    && match c.routes {
                        Some(routes) => routes
                            .iter()
                            .any(|(m, p)| *m == def.method && *p == def.path.as_str()),
                        None => !pinned,
                    }
            })
            .count();
        if matches != 1 {
            return Err(CompositionError::ConferralShapeMismatch {
                owner: member.id.clone(),
                base,
            });
        }
    }
    let mut handles = Vec::new();
    for conferral in &conferrals {
        if let Allowance::KernelHandle(handle) = conferral.allowance {
            let backed = counting
                .iter()
                .any(|c| c.group == conferral.group && c.shape == conferral.shape);
            if !backed {
                return Err(CompositionError::ConferralShapeMismatch {
                    owner: member.id.clone(),
                    base,
                });
            }
            handles.push(handle);
        }
    }
    Ok(AdmittedRouteSet {
        owner: member.id.clone(),
        standing: member.standing.tag(),
        set,
        handles,
    })
}

pub(super) fn admit_unconferred(
    member: &DeclaredMember,
    set: &RouteSet,
) -> Result<(), CompositionError> {
    use systemprompt_extension::router::{ApiBase, BodyLimit, WebBase};
    use systemprompt_extension::{ContextKind, HandlerVerifier};

    let base = set.base();
    let refuse = |allowance: Allowance| CompositionError::NotConferred {
        owner: member.id.clone(),
        base,
        allowance,
    };
    if let Some(prefix) = base.reserved_prefix() {
        return Err(CompositionError::ReservedPrefix {
            owner: member.id.clone(),
            prefix,
        });
    }
    match base {
        BasePath::Web(WebBase::TrackEngagement) => return Err(refuse(Allowance::WebPath)),
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(WebBase::Root) => {},
        BasePath::Api(_) => return Err(refuse(Allowance::Plain)),
    }
    for def in set.defs() {
        if let Some(verifier) = def.spec.policy.handler_verifier() {
            return Err(refuse(Allowance::HandlerVerified(verifier)));
        }
        if def.spec.context == ContextKind::HandlerResolved {
            return Err(refuse(Allowance::HandlerVerified(
                HandlerVerifier::GatewayCredential,
            )));
        }
        if def.spec.limits.body == BodyLimit::Agent {
            return Err(refuse(Allowance::BodyLimit));
        }
    }
    Ok(())
}

pub(super) fn admit_gateway_route(
    member: &DeclaredMember,
    decl: systemprompt_extension::router::GatewayRouteDecl,
) -> Result<AdmittedGatewayRoute, CompositionError> {
    let Some(conferral) = member.standing.terms().and_then(|terms| {
        terms.conferrals().find(|c| {
            c.base == BaseRef::Gateway(decl.path) && c.allowance == Allowance::GatewayRoute
        })
    }) else {
        return Err(CompositionError::UnlistedGatewayRoute {
            owner: member.id.clone(),
        });
    };
    let relative = systemprompt_models::modules::ApiPaths::gateway_relative(decl.path.path());
    if let Some(builtin) = systemprompt_extension::reserved::BUILTIN_GATEWAY_PREFIXES
        .iter()
        .copied()
        .find(|prefix| shadows(relative, prefix))
    {
        return Err(CompositionError::GatewayRouteShadowsBuiltin {
            owner: member.id.clone(),
            builtin,
        });
    }
    let shape_ok = decl.limits == conferral.shape.limits
        && decl.router.group() == conferral.group
        && decl
            .router
            .defs()
            .iter()
            .all(|def| def.spec.shape() == conferral.shape);
    if !shape_ok {
        return Err(CompositionError::ConferralShapeMismatch {
            owner: member.id.clone(),
            base: decl.router.base(),
        });
    }
    Ok(AdmittedGatewayRoute {
        owner: member.id.clone(),
        decl,
    })
}

pub(super) fn shadows(relative: &str, prefix: &str) -> bool {
    if prefix == "/" {
        return relative == "/";
    }
    relative == prefix
        || relative
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(super) fn full_path(base: BasePath, path: &str) -> String {
    let base = base.path();
    match (base, path) {
        ("/", path) => path.to_owned(),
        (base, "/") => base.to_owned(),
        (base, path) => format!("{base}{path}"),
    }
}

pub(super) fn check_route_conflicts(routes: &[AdmittedRouteSet]) -> Result<(), CompositionError> {
    let mut seen: HashSet<(http::Method, String)> = HashSet::new();
    for admitted in routes {
        for def in admitted.set.defs() {
            let key = (
                def.method.clone(),
                full_path(admitted.set.base(), def.path.as_str()),
            );
            if !seen.insert(key.clone()) {
                return Err(CompositionError::RouteConflict {
                    method: key.0,
                    path: key.1,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn check_gateway_overlaps(
    routes: &[AdmittedGatewayRoute],
) -> Result<(), CompositionError> {
    for (index, first) in routes.iter().enumerate() {
        if let Some(second) = routes[index + 1..]
            .iter()
            .find(|other| other.decl.path == first.decl.path)
        {
            return Err(CompositionError::GatewayRouteOverlap {
                first: first.owner.clone(),
                second: second.owner.clone(),
            });
        }
    }
    Ok(())
}
