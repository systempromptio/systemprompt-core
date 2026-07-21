use systemprompt_identifiers::RouteId;
use systemprompt_security::authz::resolver::{ResolveInput, resolve};
use systemprompt_security::authz::subject::{SubjectAttributes, SubjectDimension};
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
        attributes: &systemprompt_security::authz::NO_SUBJECT_ATTRIBUTES,
        dimensions: &[],
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
        rule(RuleType::USER, "test-user", Access::Deny),
        rule(RuleType::ROLE, "eng", Access::Allow),
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
        rule(RuleType::USER, "test-user", Access::Allow),
        rule(RuleType::ROLE, "eng", Access::Deny),
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
        rule(RuleType::ROLE, "eng", Access::Allow),
        rule(RuleType::ROLE, "contractor", Access::Deny),
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
    let rules = vec![rule(RuleType::ROLE, "ops", Access::Allow)];
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
    let rules = vec![rule(RuleType::ROLE, "ops", Access::Allow)];
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
    let rules = vec![rule(RuleType::USER, "test-user", Access::Allow)];
    let d = resolve(input(&e, &rules, &u, &roles, Some(false)));
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::UserAllow
        }
    );
}

// ---------------------------------------------------------------------------
// Extension-declared subject dimensions.
//
// The resolver learns no tenant vocabulary: a dimension is described by a
// `SubjectDimension` the caller passes in, and the subject's values for it
// arrive in `SubjectAttributes`. These cases stand in for the template's
// `department` dimension without core knowing the word.
// ---------------------------------------------------------------------------

fn department() -> RuleType {
    RuleType::extension("department").expect("well-formed slug")
}

// Precedence 100 slots between USER (0) and ROLE (200): a department rule
// outranks a role rule and yields to a user rule.
fn department_dimension() -> SubjectDimension {
    SubjectDimension {
        rule_type: department(),
        label: "Department",
        precedence: 100,
    }
}

fn in_department(name: &str) -> SubjectAttributes {
    let mut attributes = SubjectAttributes::new();
    attributes.insert(department(), vec![name.to_owned()]);
    attributes
}

fn resolve_with<'a>(
    entity: &'a EntityRef,
    rules: &'a [AccessRule],
    user_id: &'a systemprompt_identifiers::UserId,
    user_roles: &'a [String],
    attributes: &'a SubjectAttributes,
    dimensions: &'a [SubjectDimension],
) -> Decision {
    resolve(ResolveInput {
        entity,
        rules,
        user_id,
        user_roles,
        default_included: Some(false),
        parents: &[],
        attributes,
        dimensions,
    })
}

#[test]
fn user_allow_outranks_department_deny() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec![];
    let rules = vec![
        rule(department(), "eng", Access::Deny),
        rule(RuleType::USER, "test-user", Access::Allow),
    ];
    let d = resolve_with(
        &e,
        &rules,
        &u,
        &roles,
        &in_department("eng"),
        &[department_dimension()],
    );
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::UserAllow
        }
    );
}

#[test]
fn department_deny_outranks_role_allow() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![
        rule(RuleType::ROLE, "eng", Access::Allow),
        rule(department(), "finance", Access::Deny),
    ];
    let d = resolve_with(
        &e,
        &rules,
        &u,
        &roles,
        &in_department("finance"),
        &[department_dimension()],
    );
    assert_eq!(
        d,
        Decision::Deny {
            reason: DenyReason::AttributeDeny {
                entity: e.clone(),
                rule_type: department(),
                value: "finance".to_owned(),
                justification: None,
            }
        }
    );
}

#[test]
fn department_allow_outranks_role_deny() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![
        rule(RuleType::ROLE, "eng", Access::Deny),
        rule(department(), "finance", Access::Allow),
    ];
    let d = resolve_with(
        &e,
        &rules,
        &u,
        &roles,
        &in_department("finance"),
        &[department_dimension()],
    );
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::AttributeAllow {
                rule_type: department(),
                value: "finance".to_owned(),
            }
        }
    );
}

// A department rule the subject does not hold the value for is unmatchable,
// so the ladder falls through to the role band.
#[test]
fn department_rule_for_another_department_does_not_match() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![
        rule(department(), "finance", Access::Deny),
        rule(RuleType::ROLE, "eng", Access::Allow),
    ];
    let d = resolve_with(
        &e,
        &rules,
        &u,
        &roles,
        &in_department("engineering"),
        &[department_dimension()],
    );
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow {
                role: "eng".to_owned()
            }
        }
    );
}

// The regression guard for the ladder rewrite: with the dimension left
// unregistered its rules are inert, which is exactly the pre-change
// behaviour every other case in this file asserts.
#[test]
fn unregistered_dimension_rules_are_inert() {
    let e = entity();
    let u = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let rules = vec![
        rule(department(), "finance", Access::Deny),
        rule(RuleType::ROLE, "eng", Access::Allow),
    ];
    let d = resolve_with(&e, &rules, &u, &roles, &in_department("finance"), &[]);
    assert_eq!(
        d,
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow {
                role: "eng".to_owned()
            }
        }
    );
}

#[test]
fn extension_slug_rejects_core_builtins_and_malformed_input() {
    assert!(RuleType::extension("user").is_err());
    assert!(RuleType::extension("role").is_err());
    assert!(RuleType::extension("Department").is_err());
    assert!(RuleType::extension("cost centre").is_err());
    assert!(RuleType::extension("").is_err());
    assert!(RuleType::extension("cost_centre").is_ok());
}
