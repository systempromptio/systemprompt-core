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

/// A parent entity whose rules cascade onto the child being resolved.
///
/// Parents are ordered nearest-first: the entity directly above the child
/// comes before its grandparent, so a closer grant wins over a more distant
/// one within the same precedence band.
#[derive(Debug, Clone, Copy)]
pub struct ResolveParent<'a> {
    pub entity: &'a EntityRef,
    pub rules: &'a [AccessRule],
    pub default_included: Option<bool>,
}

/// Inputs to [`resolve`]. Bundled so the function stays under the clippy
/// argument-count limit and so call sites can read top-to-bottom.
#[derive(Debug, Clone, Copy)]
pub struct ResolveInput<'a> {
    pub entity: &'a EntityRef,
    pub rules: &'a [AccessRule],
    pub user_id: &'a UserId,
    pub user_roles: &'a [String],
    pub default_included: Option<bool>,
    pub parents: &'a [ResolveParent<'a>],
}

/// Resolve an access decision with deny-overrides precedence and
/// parent-entity inheritance.
///
/// The precedence ladder, evaluated strictly top to bottom and short-circuiting
/// on the first match, is:
///
/// 1. own user-deny → own user-allow → own role-deny → own role-allow
/// 2. for each parent (nearest first): parent user-deny → parent user-allow →
///    parent role-deny → parent role-allow
/// 3. own `default_included` (`true` → Allow)
/// 4. each parent's `default_included` (nearest `true` → Allow)
/// 5. otherwise Deny
///
/// A grant on a parent therefore cascades to the child only when neither the
/// child nor a nearer parent has a more specific matching rule. A child deny
/// always overrides a parent allow, and a nearer rule always overrides a
/// farther one within the same band.
///
/// If the child entity's own `default_included` is `None`, the entity is
/// unknown to access control. In that case the result is
/// [`DenyReason::UnknownEntity`] only when no rule (own or parent) matches and
/// no parent grants access via its own `default_included`; an explicit or
/// inherited grant still resolves to [`Decision::Allow`].
#[must_use]
pub fn resolve(input: ResolveInput<'_>) -> Decision {
    let ResolveInput {
        entity,
        rules,
        user_id,
        user_roles,
        default_included,
        parents,
    } = input;

    if let Some(decision) = match_ruleset(entity, rules, user_id, user_roles) {
        return decision;
    }
    for parent in parents {
        if let Some(decision) = match_ruleset(parent.entity, parent.rules, user_id, user_roles) {
            return decision;
        }
    }

    if default_included == Some(true) {
        return Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        };
    }
    if parents
        .iter()
        .any(|parent| parent.default_included == Some(true))
    {
        return Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        };
    }

    if default_included.is_none() {
        return Decision::Deny {
            reason: DenyReason::UnknownEntity {
                entity: entity.clone(),
            },
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

fn match_ruleset(
    target: &EntityRef,
    ruleset: &[AccessRule],
    user_id: &UserId,
    user_roles: &[String],
) -> Option<Decision> {
    let user_match =
        |r: &AccessRule| r.rule_type == RuleType::User && r.rule_value == user_id.as_str();
    let role_match = |r: &AccessRule| {
        r.rule_type == RuleType::Role && user_roles.iter().any(|role| role == &r.rule_value)
    };

    if let Some(rule) = ruleset
        .iter()
        .find(|r| user_match(r) && r.access == Access::Deny)
    {
        return Some(Decision::Deny {
            reason: DenyReason::UserDeny {
                entity: target.clone(),
                user_id: user_id.clone(),
                justification: rule.justification.clone(),
            },
        });
    }
    if ruleset
        .iter()
        .any(|r| user_match(r) && r.access == Access::Allow)
    {
        return Some(Decision::Allow {
            matched_by: MatchedBy::UserAllow,
        });
    }
    if let Some(rule) = ruleset
        .iter()
        .find(|r| role_match(r) && r.access == Access::Deny)
    {
        return Some(Decision::Deny {
            reason: DenyReason::RoleDeny {
                entity: target.clone(),
                role: rule.rule_value.clone(),
                justification: rule.justification.clone(),
            },
        });
    }
    if let Some(rule) = ruleset
        .iter()
        .find(|r| role_match(r) && r.access == Access::Allow)
    {
        return Some(Decision::Allow {
            matched_by: MatchedBy::RoleAllow {
                role: rule.rule_value.clone(),
            },
        });
    }
    None
}
