//! Pure deny-overrides resolver with `user > … > role` specificity.
//!
//! Core ships two subject dimensions, `user` and `role`. Extensions declare
//! their own — department, cost centre, clearance — as
//! [`SubjectDimension`]s and pass them in via [`ResolveInput::dimensions`],
//! with the user's values for them in [`ResolveInput::attributes`]. The
//! precedence ladder is built per call from those two fields, so `resolve`
//! learns no tenant vocabulary and stays pure. With no dimensions passed the
//! ladder is exactly `user > role`, which is the pre-existing behaviour.
//!
//! The function is intentionally synchronous and free of I/O so it can be
//! reused by the in-process [`super::rule_based::RuleBasedHook`], the
//! template's webhook handler, and unit tests without setup. Callers fetch
//! [`AccessRule`]s plus the `default_included` sentinel from
//! [`super::repository::AccessControlRepository`] and pass them in.
//!
//! A declared ruleset is **authoritative and closed**, at every level of the
//! parent chain: the nearest entity that declares any rule — the entity
//! itself, else its plugin, else its marketplace — decides, and a level with
//! no rules is transparent and defers upward. A narrow `roles: [admin]` on a
//! plugin therefore closes every ruleless skill and artifact it ships to
//! every other role, even when the marketplace above it admits everyone.
//!
//! `default_included` is `Option<bool>` — `None` signals the entity is
//! unknown to access control (no row in `access_control_entities`), which
//! the resolver turns into [`DenyReason::UnknownEntity`] rather than the
//! generic `NotAssigned` deny. This distinction matters operationally: an
//! unknown entity is a publish-pipeline gap, not a missing role grant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

use super::subject::{ROLE_PRECEDENCE, SubjectAttributes, SubjectDimension, USER_PRECEDENCE};
use super::types::{Access, AccessRule, Decision, DenyReason, EntityRef, MatchedBy, RuleType};

/// A parent entity whose rules cascade onto the child being resolved.
///
/// Parents are ordered nearest-first; the first parent with a non-empty
/// ruleset closes the cascade, so a farther grant can never reach past a
/// nearer, declared level.
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
    pub attributes: &'a SubjectAttributes,
    pub dimensions: &'a [SubjectDimension],
}

#[must_use]
pub fn resolve(input: ResolveInput<'_>) -> Decision {
    let ResolveInput {
        entity,
        rules,
        user_id,
        user_roles,
        default_included,
        parents,
        attributes,
        dimensions,
    } = input;

    let ladder = ladder(dimensions);
    let subject = Subject {
        user_id,
        user_roles,
        attributes,
        ladder: &ladder,
    };

    let closed = |considered: &[ResolveParent<'_>]| {
        closed_decision(entity, user_id, user_roles, default_included, considered)
    };

    if let Some(decision) = match_ruleset(entity, rules, &subject) {
        return decision;
    }
    if !rules.is_empty() {
        if default_included.is_none() {
            return Decision::Deny {
                reason: DenyReason::UnknownEntity {
                    entity: entity.clone(),
                },
            };
        }
        return closed(&[]);
    }

    for (index, parent) in parents.iter().enumerate() {
        if let Some(decision) = match_ruleset(parent.entity, parent.rules, &subject) {
            return decision;
        }
        if !parent.rules.is_empty() {
            return closed(&parents[..=index]);
        }
    }

    if default_included == Some(true)
        || parents
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
    not_assigned(entity, user_id, user_roles)
}

// Why: a declared level that did not match closes the cascade, so only the
// levels up to and including it may still admit the subject by default. The
// entity is known through that level, so an absent sentinel row is
// `NotAssigned`, never `UnknownEntity`.
fn closed_decision(
    entity: &EntityRef,
    user_id: &UserId,
    user_roles: &[String],
    default_included: Option<bool>,
    considered: &[ResolveParent<'_>],
) -> Decision {
    if default_included == Some(true)
        || considered
            .iter()
            .any(|parent| parent.default_included == Some(true))
    {
        return Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        };
    }
    not_assigned(entity, user_id, user_roles)
}

fn not_assigned(entity: &EntityRef, user_id: &UserId, user_roles: &[String]) -> Decision {
    Decision::Deny {
        reason: DenyReason::NotAssigned {
            entity: entity.clone(),
            user_id: user_id.clone(),
            roles: user_roles.to_vec(),
        },
    }
}

fn ladder(dimensions: &[SubjectDimension]) -> Vec<(RuleType, u16)> {
    let mut bands = vec![
        (RuleType::USER, USER_PRECEDENCE),
        (RuleType::ROLE, ROLE_PRECEDENCE),
    ];
    bands.extend(
        dimensions
            .iter()
            .filter(|d| d.rule_type != RuleType::USER && d.rule_type != RuleType::ROLE)
            .map(|d| (d.rule_type.clone(), d.precedence)),
    );
    bands.sort_by_key(|&(_, precedence)| precedence);
    bands
}

struct Subject<'a> {
    user_id: &'a UserId,
    user_roles: &'a [String],
    attributes: &'a SubjectAttributes,
    ladder: &'a [(RuleType, u16)],
}

impl Subject<'_> {
    fn matches(&self, rule: &AccessRule) -> bool {
        if rule.rule_type == RuleType::USER {
            return rule.rule_value == self.user_id.as_str();
        }
        let held = if rule.rule_type == RuleType::ROLE {
            self.user_roles
        } else {
            self.attributes.values(&rule.rule_type)
        };
        held.iter().any(|value| value == &rule.rule_value)
    }
}

fn match_ruleset(
    target: &EntityRef,
    ruleset: &[AccessRule],
    subject: &Subject<'_>,
) -> Option<Decision> {
    for (rule_type, _) in subject.ladder {
        let in_band = |r: &&AccessRule| r.rule_type == *rule_type && subject.matches(r);

        if let Some(rule) = ruleset
            .iter()
            .find(|r| in_band(r) && r.access == Access::Deny)
        {
            return Some(deny_for(target, subject, rule));
        }
        if let Some(rule) = ruleset
            .iter()
            .find(|r| in_band(r) && r.access == Access::Allow)
        {
            return Some(allow_for(rule));
        }
    }
    None
}

fn deny_for(target: &EntityRef, subject: &Subject<'_>, rule: &AccessRule) -> Decision {
    let reason = if rule.rule_type == RuleType::USER {
        DenyReason::UserDeny {
            entity: target.clone(),
            user_id: subject.user_id.clone(),
            justification: rule.justification.clone(),
        }
    } else if rule.rule_type == RuleType::ROLE {
        DenyReason::RoleDeny {
            entity: target.clone(),
            role: rule.rule_value.clone(),
            justification: rule.justification.clone(),
        }
    } else {
        DenyReason::AttributeDeny {
            entity: target.clone(),
            rule_type: rule.rule_type.clone(),
            value: rule.rule_value.clone(),
            justification: rule.justification.clone(),
        }
    };
    Decision::Deny { reason }
}

fn allow_for(rule: &AccessRule) -> Decision {
    let matched_by = if rule.rule_type == RuleType::USER {
        MatchedBy::UserAllow
    } else if rule.rule_type == RuleType::ROLE {
        MatchedBy::RoleAllow {
            role: rule.rule_value.clone(),
        }
    } else {
        MatchedBy::AttributeAllow {
            rule_type: rule.rule_type.clone(),
            value: rule.rule_value.clone(),
        }
    };
    Decision::Allow { matched_by }
}
