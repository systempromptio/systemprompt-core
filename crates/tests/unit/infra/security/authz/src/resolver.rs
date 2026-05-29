use systemprompt_identifiers::RouteId;
use systemprompt_security::authz::resolver::{ResolveInput, resolve};
use systemprompt_security::authz::types::{
    Access, AccessRule, Decision, DenyReason, EntityRef, MatchedBy, RuleType,
};
use systemprompt_test_fixtures::fixture_user_id;

fn rule(rule_type: RuleType, value: &str, access: Access) -> AccessRule {
    AccessRule {
        id: systemprompt_identifiers::RuleId::new(format!("{rule_type}-{value}-{access}")),
        rule_type,
        rule_value: value.into(),
        access,
        justification: None,
    }
}

fn entity() -> EntityRef {
    EntityRef::GatewayRoute(RouteId::new("test-route"))
}

fn input<'a>(
    entity: &'a EntityRef,
    rules: &'a [AccessRule],
    user_id: &'a systemprompt_identifiers::UserId,
    user_roles: &'a [String],
    default_included: Option<bool>,
) -> ResolveInput<'a> {
    ResolveInput {
        entity,
        rules,
        user_id,
        user_roles,
        default_included,
        parents: &[],
    }
}

#[test]
fn unknown_entity_denies() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let d = resolve(input(&e, &[], &u, &roles, None));
    assert!(matches!(
        d,
        Decision::Deny {
            reason: DenyReason::UnknownEntity { .. }
        }
    ));
}

#[test]
fn no_rules_no_default_denies() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let d = resolve(input(&e, &[], &u, &roles, Some(false)));
    assert!(matches!(
        d,
        Decision::Deny {
            reason: DenyReason::NotAssigned { .. }
        }
    ));
}

#[test]
fn no_rules_default_allows() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let d = resolve(input(&e, &[], &u, &roles, Some(true)));
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    );
}

#[test]
fn user_deny_overrides_role_allow() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![
        rule(RuleType::User, "test-user", Access::Deny),
        rule(RuleType::Role, "eng", Access::Allow),
    ];
    let d = resolve(input(&e, &rules, &u, &roles, Some(true)));
    assert!(
        matches!(
            d,
            Decision::Deny {
                reason: DenyReason::UserDeny { ref user_id, .. }
            } if user_id.as_str() == "test-user"
        ),
        "got {d:?}",
    );
}

#[test]
fn user_allow_beats_role_deny() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![
        rule(RuleType::User, "test-user", Access::Allow),
        rule(RuleType::Role, "eng", Access::Deny),
    ];
    let d = resolve(input(&e, &rules, &u, &roles, Some(false)));
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::UserAllow
        }
    );
}

#[test]
fn role_deny_overrides_role_allow_in_multirole() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into(), "contractor".into()];
    let rules = vec![
        rule(RuleType::Role, "eng", Access::Allow),
        rule(RuleType::Role, "contractor", Access::Deny),
    ];
    let d = resolve(input(&e, &rules, &u, &roles, Some(false)));
    assert!(
        matches!(
            d,
            Decision::Deny {
                reason: DenyReason::RoleDeny { ref role, .. }
            } if role == "contractor"
        ),
        "got {d:?}",
    );
}

#[test]
fn no_match_with_default_off_denies_not_assigned() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![rule(RuleType::Role, "ops", Access::Allow)];
    let d = resolve(input(&e, &rules, &u, &roles, Some(false)));
    assert!(matches!(
        d,
        Decision::Deny {
            reason: DenyReason::NotAssigned { .. }
        }
    ));
}

#[test]
fn no_match_with_default_on_allows() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![rule(RuleType::Role, "ops", Access::Allow)];
    let d = resolve(input(&e, &rules, &u, &roles, Some(true)));
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    );
}

#[test]
fn user_allow_alone_allows() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec![];
    let rules = vec![rule(RuleType::User, "test-user", Access::Allow)];
    let d = resolve(input(&e, &rules, &u, &roles, Some(false)));
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::UserAllow
        }
    );
}
