//! Composition-wide HTTP admission: the one site-fallback slot, and the
//! refusal of bearer-authenticated routes in a composition without `oauth`.
//!
//! A composition holds at most one [`SiteFallback`]; a second claimant, from
//! another member or the same one, is refused naming both. A route set def
//! whose policy admits no anonymous caller and names no handler verifier is
//! bearer-authenticated, and so is every gateway route (each is verified by a
//! core-issued gateway credential): it needs the tokens the `oauth` member
//! issues, so a composition whose `oauth` member is disabled, removed or not
//! linked is refused at admission with every member that declares such a route.
//! A composition with no bearer-authenticated route (a CLI-only or worker
//! composition) admits without `oauth`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(feature = "api")]

use systemprompt_extension::admitted::{
    AdmittedGatewayRoute, AdmittedRouteSet, AdmittedSiteFallback,
};
use systemprompt_extension::router::{RouteDef, SiteFallback};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::UserType;

use super::DeclaredMember;
use super::error::CompositionError;

pub(super) const OAUTH_MEMBER_ID: &str = "oauth";

pub(super) fn admit_site_fallback(
    held: Option<AdmittedSiteFallback>,
    member: &DeclaredMember,
    fallback: SiteFallback,
) -> Result<AdmittedSiteFallback, CompositionError> {
    if let Some(first) = held {
        return Err(CompositionError::SecondSiteFallback {
            first: first.owner,
            second: member.id.clone(),
        });
    }
    Ok(AdmittedSiteFallback {
        owner: member.id.clone(),
        standing: member.standing.tag(),
        fallback,
    })
}

fn bearer_authenticated(def: &RouteDef) -> bool {
    def.spec.policy.handler_verifier().is_none() && !def.spec.policy.permits(UserType::Anon)
}

pub(super) fn refuse_bearer_routes_without_oauth(
    routes: &[AdmittedRouteSet],
    gateway_routes: &[AdmittedGatewayRoute],
) -> Result<(), CompositionError> {
    let mut members: Vec<ExtensionId> = Vec::new();
    let bearing = routes
        .iter()
        .filter(|admitted| admitted.set.defs().iter().any(bearer_authenticated))
        .map(|admitted| &admitted.owner)
        .chain(gateway_routes.iter().map(|route| &route.owner));
    for owner in bearing {
        if !members.contains(owner) {
            members.push(owner.clone());
        }
    }
    if members.is_empty() {
        return Ok(());
    }
    Err(CompositionError::BearerRoutesWithoutOauth { members })
}
