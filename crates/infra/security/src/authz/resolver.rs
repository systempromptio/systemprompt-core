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
//! A declared ruleset is **authoritative and closed**: an entity that names its
//! own roles is closed to every role it does not name, and only an entity with
//! no rules of its own defers to its parents. This is what makes a narrow
//! `roles: [admin]` grant restrictive even when the entity belongs to a group
//! that is granted to everyone.
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
    /// The user's values for each extension-declared dimension, gathered by
    /// [`gather_subject_attributes`][super::subject::gather_subject_attributes].
    pub attributes: &'a SubjectAttributes,
    /// Extension dimensions to interleave into the precedence ladder. Passed
    /// in rather than read from the inventory so `resolve` stays pure and
    /// unit-testable; pass `&[]` for core-only `user > role` behaviour.
    pub dimensions: &'a [SubjectDimension],
}

/// Resolves a decision with parent inheritance on the crate-head deny-overrides
/// model.
///
/// A child deny overrides a parent allow, a nearer rule overrides a farther one
/// within the same precedence band, and a parent grant cascades to the child
/// only when the child declares no rules at all — a child that declares any
/// rule owns its decision and is closed to roles it does not name. An unknown
/// child entity (`default_included == None`) yields
/// [`DenyReason::UnknownEntity`] unless a rule or a parent's `default_included`
/// grants access.
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

    if let Some(decision) = match_ruleset(entity, rules, &subject) {
        return decision;
    }
    // A declared ruleset is authoritative: an entity that names its own roles is
    // closed to every role it does not name. `match_ruleset` cannot distinguish
    // "no rule matches you" from "no rules exist", so only an entity with no rules
    // of its own defers to its parents.
    let parents = if rules.is_empty() { parents } else { &[] };

    for parent in parents {
        if let Some(decision) = match_ruleset(parent.entity, parent.rules, &subject) {
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

/// The precedence ladder for one call: core's built-ins unioned with the
/// caller-supplied dimensions, tightest-binding first.
///
/// A dimension that re-declares `user` or `role` is ignored rather than
/// duplicating a band — core owns those two slugs and their precedence.
/// The sort is stable, so dimensions sharing a precedence keep registration
/// order.
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

/// Everything about the requesting subject the band matcher needs, bundled so
/// the ladder is built once per [`resolve`] rather than once per ruleset.
struct Subject<'a> {
    user_id: &'a UserId,
    user_roles: &'a [String],
    attributes: &'a SubjectAttributes,
    ladder: &'a [(RuleType, u16)],
}

impl Subject<'_> {
    /// Whether `rule` targets this subject in its own band.
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

/// Walks the precedence ladder tightest band first, denying before allowing
/// within each band. The first band that matches decides; a band the subject
/// holds no value for cannot match and falls through.
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

/// Built-ins keep their dedicated variants so existing `governance_decisions`
/// audit JSON stays stable; extension dimensions report through the generic
/// attribute variants.
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
