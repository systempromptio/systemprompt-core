//! Pure deny-overrides resolver with `user > role` specificity.
//!
//! The function is intentionally synchronous and free of I/O so it can be
//! reused by the in-process [`super::rule_based::RuleBasedHook`], the
//! template's webhook handler, and unit tests without setup. Callers fetch
//! [`AccessRule`]s plus the `default_included` sentinel from
//! [`super::repository::AccessControlRepository`] and pass them in.
//!
//! `default_included` is `Option<bool>` — `None` signals the entity is
//! unknown to access control (no row in `access_control_entities`), which
//! the resolver turns into [`DenyReason::UnknownEntity`] rather than the
//! generic `NotAssigned` deny. This distinction matters operationally: an
//! unknown entity is a publish-pipeline gap, not a missing role grant.

use systemprompt_identifiers::UserId;

use super::types::{Access, AccessRule, Decision, DenyReason, EntityRef, MatchedBy, RuleType};

/// Inputs to [`resolve`]. Bundled so the function stays under the clippy
/// argument-count limit and so call sites can read top-to-bottom.
#[derive(Debug, Clone, Copy)]
pub struct ResolveInput<'a> {
    pub entity: &'a EntityRef,
    pub rules: &'a [AccessRule],
    pub user_id: &'a UserId,
    pub user_roles: &'a [String],
    pub default_included: Option<bool>,
}

#[must_use]
pub fn resolve(input: ResolveInput<'_>) -> Decision {
    let ResolveInput {
        entity,
        rules,
        user_id,
        user_roles,
        default_included,
    } = input;
    let Some(default_included) = default_included else {
        return Decision::Deny {
            reason: DenyReason::UnknownEntity {
                entity: entity.clone(),
            },
        };
    };

    let user_match =
        |r: &AccessRule| r.rule_type == RuleType::User && r.rule_value == user_id.as_str();
    let role_match = |r: &AccessRule| {
        r.rule_type == RuleType::Role && user_roles.iter().any(|role| role == &r.rule_value)
    };

    if let Some(rule) = rules
        .iter()
        .find(|r| user_match(r) && r.access == Access::Deny)
    {
        return Decision::Deny {
            reason: DenyReason::UserDeny {
                entity: entity.clone(),
                user_id: user_id.clone(),
                justification: rule.justification.clone(),
            },
        };
    }
    if rules
        .iter()
        .any(|r| user_match(r) && r.access == Access::Allow)
    {
        return Decision::Allow {
            matched_by: MatchedBy::UserAllow,
        };
    }
    if let Some(rule) = rules
        .iter()
        .find(|r| role_match(r) && r.access == Access::Deny)
    {
        return Decision::Deny {
            reason: DenyReason::RoleDeny {
                entity: entity.clone(),
                role: rule.rule_value.clone(),
                justification: rule.justification.clone(),
            },
        };
    }
    if let Some(rule) = rules
        .iter()
        .find(|r| role_match(r) && r.access == Access::Allow)
    {
        return Decision::Allow {
            matched_by: MatchedBy::RoleAllow {
                role: rule.rule_value.clone(),
            },
        };
    }
    if default_included {
        return Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        };
    }
    Decision::Deny {
        reason: DenyReason::NotAssigned {
            entity: entity.clone(),
            user_id: user_id.clone(),
            roles: user_roles.to_vec(),
        },
    }
}
