//! Pure deny-overrides resolver with `user > role > department` specificity.
//!
//! The function is intentionally synchronous and free of I/O so it can be
//! reused by the in-process default hook, the template's webhook handler,
//! and unit tests without setup. Callers fetch [`AccessRule`]s plus the
//! `default_included` sentinel from
//! [`super::repository::AccessControlRepository`] and pass them in.

use systemprompt_identifiers::UserId;

use super::types::{Access, AccessRule, Decision, RuleType};

#[must_use]
pub fn resolve(
    rules: &[AccessRule],
    user_id: &UserId,
    user_roles: &[String],
    department: &str,
    default_included: bool,
) -> Decision {
    let user_match =
        |r: &&AccessRule| r.rule_type == RuleType::User && r.rule_value == user_id.as_str();
    let role_match = |r: &&AccessRule| {
        r.rule_type == RuleType::Role && user_roles.iter().any(|role| role == &r.rule_value)
    };
    let dept_match = |r: &&AccessRule| {
        r.rule_type == RuleType::Department && r.rule_value == department && !department.is_empty()
    };

    if let Some(rule) = rules
        .iter()
        .find(|r| user_match(r) && r.access == Access::Deny)
    {
        return Decision::Deny {
            reason: format!("user-level deny: {}", user_id.as_str()),
            justification: rule.justification.clone(),
        };
    }
    if rules
        .iter()
        .any(|r| user_match(&r) && r.access == Access::Allow)
    {
        return Decision::Allow;
    }
    if let Some(rule) = rules
        .iter()
        .find(|r| role_match(r) && r.access == Access::Deny)
    {
        return Decision::Deny {
            reason: format!("role deny: {}", rule.rule_value),
            justification: rule.justification.clone(),
        };
    }
    if rules
        .iter()
        .any(|r| role_match(&r) && r.access == Access::Allow)
    {
        return Decision::Allow;
    }
    if let Some(rule) = rules
        .iter()
        .find(|r| dept_match(r) && r.access == Access::Deny)
    {
        return Decision::Deny {
            reason: format!("department deny: {department}"),
            justification: rule.justification.clone(),
        };
    }
    if rules
        .iter()
        .any(|r| dept_match(&r) && r.access == Access::Allow)
    {
        return Decision::Allow;
    }
    if default_included {
        return Decision::Allow;
    }
    Decision::Deny {
        reason: "not assigned".into(),
        justification: None,
    }
}
