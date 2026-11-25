//! The capabilities a member declared it reads in place of a sealed seed
//! (`Contributions::conferral`, `Contributions::token_audience`), admitted
//! here and inserted sealed as one [`ConferralDeclarations`].
//!
//! A declared token audience is refused when its namespace is a core audience
//! (`mcp`, `api`, …) or a core member id, and unless it is in the member's own
//! namespace (`<member-id>:<name>`, the name 1 to 64 characters of
//! `[a-z0-9._-]`), is not a core audience, and is not an audience the
//! runtime configuration accepts (`jwt_audiences`,
//! `allowed_resource_audiences`); a declared scope is refused when it is a
//! core permission (`admin` among them) or is not 1 to 64 characters of
//! `[a-z0-9._:-]`. Each refusal names the member, the value and the reason,
//! so boot reports it before any provide fn runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::{BuiltState, ConferralDeclarations, MemberConferrals, Resolved};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{is_core_audience, is_core_scope};
use systemprompt_traits::TokenAudienceGrant;

use super::KernelError;

const NAME_MAX: usize = 64;

const RESERVED_HOP_NAMESPACES: &[&str] = &["owner-hop", "mcp"];

fn audience_refusal(owner: &str, audience: &str, config: Option<&Config>) -> Option<&'static str> {
    if is_core_audience(owner)
        || CoreId::ALL.iter().any(|core| core.id() == owner)
        || RESERVED_HOP_NAMESPACES.contains(&owner)
    {
        return Some("a core audience or core member id is never an audience namespace");
    }
    let Some(name) = audience
        .strip_prefix(owner)
        .and_then(|rest| rest.strip_prefix(':'))
    else {
        return Some("an audience is named <member-id>:<name>");
    };
    let valid = !name.is_empty()
        && name.len() <= NAME_MAX
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if !valid {
        return Some("an audience name is 1 to 64 characters of [a-z0-9._-]");
    }
    if is_core_audience(audience) {
        return Some("it is a core audience");
    }
    let configured = config.is_some_and(|config| {
        config
            .allowed_resource_audiences
            .iter()
            .any(|accepted| accepted.eq_ignore_ascii_case(audience))
            || config
                .jwt_audiences
                .iter()
                .any(|accepted| accepted.as_str().eq_ignore_ascii_case(audience))
    });
    configured.then_some("the runtime configuration accepts it")
}

fn scope_refusal(scope: &str) -> Option<&'static str> {
    if is_core_scope(scope) {
        return Some("it is a core permission");
    }
    let valid = !scope.is_empty()
        && scope.len() <= NAME_MAX
        && scope.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b':' | b'-')
        });
    (!valid).then_some("a scope is 1 to 64 characters of [a-z0-9._:-]")
}

fn admitted_audiences(
    owner: &systemprompt_identifiers::ExtensionId,
    declared: &[TokenAudienceGrant],
    config: Option<&Config>,
) -> Result<(), KernelError> {
    for grant in declared {
        if let Some(reason) = audience_refusal(owner.as_str(), &grant.audience, config) {
            return Err(KernelError::ConferredAudience {
                owner: owner.clone(),
                audience: grant.audience.clone(),
                reason,
            });
        }
        if let Some((scope, reason)) = grant
            .scopes
            .iter()
            .find_map(|scope| scope_refusal(scope).map(|reason| (scope, reason)))
        {
            return Err(KernelError::ConferredScope {
                owner: owner.clone(),
                audience: grant.audience.clone(),
                scope: scope.clone(),
                reason,
            });
        }
    }
    Ok(())
}

pub(super) fn declarations(
    built: &BuiltState,
    r: &Resolved<'_>,
) -> Result<ConferralDeclarations, KernelError> {
    let config = r.optional::<Arc<Config>>();
    let mut declared = HashMap::new();
    for member in built.members() {
        let audiences = member.sink.declared_token_audiences();
        admitted_audiences(&member.owner, audiences, config.as_deref())?;
        declared.insert(
            member.owner.clone(),
            MemberConferrals {
                rows: Arc::from(member.sink.declared_conferrals().to_vec()),
                token_audiences: Arc::from(audiences.to_vec()),
            },
        );
    }
    Ok(ConferralDeclarations::new(declared))
}
