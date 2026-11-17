//! Route-set and gateway-route admission against the conferrals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(feature = "api")]

use systemprompt_extension::admitted::{AdmittedGatewayRoute, AdmittedRouteSet};
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, GatewayDefRefusal, RouteSet, WebBase, first_overlap,
    path_covers, refused_gateway_def, refused_reservation,
};
use systemprompt_extension::{ContributionError, StandingTag};

use super::error::CompositionError;
use super::{Conferral, DeclaredMember};

pub(super) fn admit_route_set(
    member: &DeclaredMember,
    set: RouteSet,
) -> Result<AdmittedRouteSet, CompositionError> {
    let base = set.base();
    let BaseRef::Path(path) = base else {
        return Err(wrong_kind(member, base));
    };
    let conferrals: Vec<&'static Conferral> = member
        .standing
        .terms()
        .map(|terms| terms.conferrals().filter(|c| c.base == base).collect())
        .unwrap_or_default();
    admit_services(member, path, &set, conferrals.is_empty())?;
    if conferrals.is_empty() {
        admit_unconferred(member, path, &set)?;
        admit_reserved(member, &set)?;
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
    admit_reserved(member, &set)?;
    Ok(AdmittedRouteSet {
        owner: member.id.clone(),
        standing: member.standing.tag(),
        set,
        handles,
    })
}

fn admit_services(
    member: &DeclaredMember,
    base: BasePath,
    set: &RouteSet,
    unconferred: bool,
) -> Result<(), CompositionError> {
    if !set.defs().iter().any(|def| def.is_service()) {
        return Ok(());
    }
    let open_base = matches!(
        base,
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(WebBase::Root)
    );
    if member.standing.tag() != StandingTag::Install || !unconferred || !open_base {
        return Err(CompositionError::ServiceNotAdmitted {
            owner: member.id.clone(),
            base,
        });
    }
    Ok(())
}

fn admit_reserved(member: &DeclaredMember, set: &RouteSet) -> Result<(), CompositionError> {
    let holder = match member.standing.tag() {
        StandingTag::Core => CoreId::for_id(member.id.as_str()),
        StandingTag::Install => None,
    };
    for def in set.defs() {
        if let Some(reserved) = refused_reservation(set.base(), def, holder) {
            return Err(CompositionError::ReservedPrefix {
                owner: member.id.clone(),
                prefix: reserved.path,
            });
        }
    }
    Ok(())
}

fn wrong_kind(member: &DeclaredMember, base: BaseRef) -> CompositionError {
    CompositionError::Contribution {
        owner: member.id.clone(),
        source: ContributionError::RouteSetKind(base),
    }
}

fn admit_unconferred(
    member: &DeclaredMember,
    base: BasePath,
    set: &RouteSet,
) -> Result<(), CompositionError> {
    use systemprompt_extension::router::BodyLimit;
    use systemprompt_extension::{ContextKind, HandlerVerifier};

    let refuse = |allowance: Allowance| CompositionError::NotConferred {
        owner: member.id.clone(),
        base,
        allowance,
    };
    if base == BasePath::Web(WebBase::TrackEngagement) {
        return Err(refuse(Allowance::WebPath));
    }
    if let Some(prefix) = base.reserved_prefix() {
        return Err(CompositionError::ReservedPrefix {
            owner: member.id.clone(),
            prefix,
        });
    }
    match base {
        BasePath::Api(ApiBase::Extension(_)) | BasePath::Web(_) => {},
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
    let Some(path) = decl.path() else {
        return Err(wrong_kind(member, decl.router.base()));
    };
    let base = BaseRef::Gateway(path);
    let Some(conferral) = member.standing.terms().and_then(|terms| {
        terms
            .conferrals()
            .find(|c| c.base == base && c.allowance == Allowance::GatewayRoute)
    }) else {
        return Err(CompositionError::UnlistedGatewayRoute {
            owner: member.id.clone(),
        });
    };
    let relative = systemprompt_models::modules::ApiPaths::gateway_relative(path.path());
    if let Some(builtin) = systemprompt_extension::reserved::BUILTIN_GATEWAY_PREFIXES
        .iter()
        .copied()
        .find(|prefix| path_covers(relative, prefix))
    {
        return Err(CompositionError::GatewayRouteShadowsBuiltin {
            owner: member.id.clone(),
            builtin,
        });
    }
    for def in decl.router.defs() {
        match refused_gateway_def(path, def) {
            Some(GatewayDefRefusal::UnderBuiltin { path, builtin }) => {
                return Err(CompositionError::GatewayRouteUnderBuiltin {
                    owner: member.id.clone(),
                    path,
                    builtin,
                });
            },
            Some(GatewayDefRefusal::OutsideClaim { path, claimed }) => {
                return Err(CompositionError::GatewayRouteOutsideClaim {
                    owner: member.id.clone(),
                    path,
                    claimed,
                });
            },
            None => {},
        }
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
            base,
        });
    }
    Ok(AdmittedGatewayRoute {
        owner: member.id.clone(),
        decl,
    })
}

pub(super) fn check_route_conflicts(routes: &[AdmittedRouteSet]) -> Result<(), CompositionError> {
    let defs = routes
        .iter()
        .flat_map(|admitted| admitted.set.defs().iter().map(|def| (admitted.set.base(), def)));
    match first_overlap(defs) {
        Some(overlap) => Err(CompositionError::RouteConflict {
            method: overlap.method,
            path: overlap.path,
        }),
        None => Ok(()),
    }
}

pub(super) fn check_gateway_overlaps(
    routes: &[AdmittedGatewayRoute],
) -> Result<(), CompositionError> {
    for (index, first) in routes.iter().enumerate() {
        if let Some(second) = routes[index + 1..]
            .iter()
            .find(|other| other.decl.router.base() == first.decl.router.base())
        {
            return Err(CompositionError::GatewayRouteOverlap {
                first: first.owner.clone(),
                second: second.owner.clone(),
            });
        }
    }
    let defs = routes.iter().flat_map(|route| {
        let set = &route.decl.router;
        set.defs().iter().map(|def| (set.base(), def))
    });
    match first_overlap(defs) {
        Some(overlap) => Err(CompositionError::RouteConflict {
            method: overlap.method,
            path: overlap.path,
        }),
        None => Ok(()),
    }
}
