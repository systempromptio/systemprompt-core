use systemprompt_security::authz::resolver::resolve;
use systemprompt_security::authz::types::{Access, AccessRule, Decision, RuleType};
use systemprompt_test_fixtures::fixture_user_id;

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
    let d = resolve(&[], &fixture_user_id(), &["eng".into()], "platform", false);
    assert!(matches!(d, Decision::Deny { .. }));
}

#[test]
fn no_rules_default_allows() {
    let d = resolve(&[], &fixture_user_id(), &["eng".into()], "platform", true);
    assert_eq!(d, Decision::Allow);
}

#[test]
fn user_deny_overrides_role_allow() {
    let rules = vec![
        rule(RuleType::User, "test-user", Access::Deny),
        rule(RuleType::Role, "eng", Access::Allow),
    ];
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        true,
    );
    assert!(
        matches!(d, Decision::Deny { ref reason, .. } if reason == "user-level deny: test-user"),
        "got {d:?}",
    );
}

#[test]
fn user_allow_beats_role_deny() {
    let rules = vec![
        rule(RuleType::User, "test-user", Access::Allow),
        rule(RuleType::Role, "eng", Access::Deny),
    ];
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        false,
    );
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
        &fixture_user_id(),
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
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        false,
    );
    assert_eq!(d, Decision::Allow);
}

#[test]
fn department_deny_when_no_role_match() {
    let rules = vec![rule(RuleType::Department, "platform", Access::Deny)];
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        true,
    );
    assert!(
        matches!(d, Decision::Deny { ref reason, .. } if reason == "department deny: platform"),
    );
}

#[test]
fn department_allow_when_no_role_match() {
    let rules = vec![rule(RuleType::Department, "platform", Access::Allow)];
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        false,
    );
    assert_eq!(d, Decision::Allow);
}

#[test]
fn empty_department_does_not_match_dept_rules() {
    let rules = vec![rule(RuleType::Department, "", Access::Allow)];
    let d = resolve(&rules, &fixture_user_id(), &["eng".into()], "", false);
    assert!(matches!(d, Decision::Deny { ref reason, .. } if reason == "not assigned"));
}

#[test]
fn no_match_with_default_off_denies_not_assigned() {
    let rules = vec![rule(RuleType::Role, "ops", Access::Allow)];
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        false,
    );
    assert!(matches!(d, Decision::Deny { ref reason, .. } if reason == "not assigned"));
}

#[test]
fn no_match_with_default_on_allows() {
    let rules = vec![rule(RuleType::Role, "ops", Access::Allow)];
    let d = resolve(
        &rules,
        &fixture_user_id(),
        &["eng".into()],
        "platform",
        true,
    );
    assert_eq!(d, Decision::Allow);
}

#[test]
fn user_allow_alone_allows() {
    let rules = vec![rule(RuleType::User, "test-user", Access::Allow)];
    let d = resolve(&rules, &fixture_user_id(), &[], "", false);
    assert_eq!(d, Decision::Allow);
}
