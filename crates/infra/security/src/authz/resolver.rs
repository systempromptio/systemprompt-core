//! Pure deny-overrides resolver with `user > role > department` specificity.
//!
//! The function is intentionally synchronous and free of I/O so it can be
//! reused by the in-process default hook, the template's webhook handler,
//! and unit tests without setup. Callers fetch [`AccessRule`]s plus the
//! `default_included` sentinel from
//! [`super::repository::AccessControlRepository`] and pass them in.

use super::types::{Access, AccessRule, Decision, RuleType};

#[must_use]
pub fn resolve(
    rules: &[AccessRule],
    user_id: &str,
    user_roles: &[String],
    department: &str,
    default_included: bool,
) -> Decision {
    let user_match = |r: &&AccessRule| r.rule_type == RuleType::User && r.rule_value == user_id;
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
            reason: format!("user-level deny: {user_id}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(rule_type: RuleType, value: &str, access: Access) -> AccessRule {
        AccessRule {
            id: systemprompt_identifiers::RuleId::new(format!("{rule_type}-{value}-{access}")),
            rule_type,
            rule_value: value.into(),
            access,
            default_included: false,
            justification: None,
        }
    }

    #[test]
    fn no_rules_no_default_denies() {
        let d = resolve(&[], "u1", &["eng".into()], "platform", false);
        assert!(matches!(d, Decision::Deny { .. }));
    }

    #[test]
    fn no_rules_default_allows() {
        let d = resolve(&[], "u1", &["eng".into()], "platform", true);
        assert_eq!(d, Decision::Allow);
    }

    #[test]
    fn user_deny_overrides_role_allow() {
        let rules = vec![
            rule(RuleType::User, "u1", Access::Deny),
            rule(RuleType::Role, "eng", Access::Allow),
        ];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", true);
        assert!(
            matches!(d, Decision::Deny { ref reason, .. } if reason == "user-level deny: u1"),
            "got {d:?}",
        );
    }

    #[test]
    fn user_allow_beats_role_deny() {
        let rules = vec![
            rule(RuleType::User, "u1", Access::Allow),
            rule(RuleType::Role, "eng", Access::Deny),
        ];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", false);
        assert_eq!(d, Decision::Allow);
    }

    #[test]
    fn role_deny_overrides_role_allow_in_multirole() {
        let rules = vec![
            rule(RuleType::Role, "eng", Access::Allow),
            rule(RuleType::Role, "contractor", Access::Deny),
        ];
        let d = resolve(
            &rules,
            "u1",
            &["eng".into(), "contractor".into()],
            "platform",
            false,
        );
        assert!(
            matches!(d, Decision::Deny { ref reason, .. } if reason == "role deny: contractor"),
            "got {d:?}",
        );
    }

    #[test]
    fn role_allow_beats_department_deny() {
        let rules = vec![
            rule(RuleType::Role, "eng", Access::Allow),
            rule(RuleType::Department, "platform", Access::Deny),
        ];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", false);
        assert_eq!(d, Decision::Allow);
    }

    #[test]
    fn department_deny_when_no_role_match() {
        let rules = vec![rule(RuleType::Department, "platform", Access::Deny)];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", true);
        assert!(
            matches!(d, Decision::Deny { ref reason, .. } if reason == "department deny: platform"),
        );
    }

    #[test]
    fn department_allow_when_no_role_match() {
        let rules = vec![rule(RuleType::Department, "platform", Access::Allow)];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", false);
        assert_eq!(d, Decision::Allow);
    }

    #[test]
    fn empty_department_does_not_match_dept_rules() {
        let rules = vec![rule(RuleType::Department, "", Access::Allow)];
        let d = resolve(&rules, "u1", &["eng".into()], "", false);
        assert!(matches!(d, Decision::Deny { ref reason, .. } if reason == "not assigned"));
    }

    #[test]
    fn no_match_with_default_off_denies_not_assigned() {
        let rules = vec![rule(RuleType::Role, "ops", Access::Allow)];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", false);
        assert!(matches!(d, Decision::Deny { ref reason, .. } if reason == "not assigned"));
    }

    #[test]
    fn no_match_with_default_on_allows() {
        let rules = vec![rule(RuleType::Role, "ops", Access::Allow)];
        let d = resolve(&rules, "u1", &["eng".into()], "platform", true);
        assert_eq!(d, Decision::Allow);
    }

    #[test]
    fn user_allow_alone_allows() {
        let rules = vec![rule(RuleType::User, "u1", Access::Allow)];
        let d = resolve(&rules, "u1", &[], "", false);
        assert_eq!(d, Decision::Allow);
    }
}
