use systemprompt_identifiers::{MarketplaceId, SkillId};
use systemprompt_security::authz::{ResolveInput, ResolveParent, resolve};
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

fn child() -> EntityRef {
    EntityRef::Skill(SkillId::new("child-skill"))
}

fn parent_entity() -> EntityRef {
    EntityRef::Marketplace(MarketplaceId::new("parent-market"))
}

#[test]
fn own_allow_wins_over_parent() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let own = vec![rule(RuleType::Role, "eng", Access::Allow)];
    let parent_rules = vec![rule(RuleType::Role, "eng", Access::Deny)];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &parent_rules,
        default_included: Some(false),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &own,
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &parents,
    });

    assert_eq!(
        decision,
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow { role: "eng".into() }
        }
    );
}

#[test]
fn own_deny_overrides_parent_allow() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let own = vec![rule(RuleType::Role, "eng", Access::Deny)];
    let parent_rules = vec![rule(RuleType::Role, "eng", Access::Allow)];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &parent_rules,
        default_included: Some(true),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &own,
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &parents,
    });

    assert!(
        matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::RoleDeny { ref role, .. }
            } if role == "eng"
        ),
        "got {decision:?}",
    );
}

#[test]
fn parent_allow_cascades_when_own_absent() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let parent_rules = vec![rule(RuleType::Role, "eng", Access::Allow)];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &parent_rules,
        default_included: Some(false),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &parents,
    });

    assert_eq!(
        decision,
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow { role: "eng".into() }
        }
    );
}

#[test]
fn parent_deny_overrides_parent_allow() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into(), "contractor".into()];
    let parent_rules = vec![
        rule(RuleType::Role, "eng", Access::Allow),
        rule(RuleType::Role, "contractor", Access::Deny),
    ];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &parent_rules,
        default_included: Some(false),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &parents,
    });

    assert!(
        matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::RoleDeny { ref role, .. }
            } if role == "contractor"
        ),
        "got {decision:?}",
    );
}

#[test]
fn nearer_parent_allow_beats_farther_parent_deny() {
    let child = child();
    let near = EntityRef::Marketplace(MarketplaceId::new("near-market"));
    let far = EntityRef::Marketplace(MarketplaceId::new("far-market"));
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let near_rules = vec![rule(RuleType::Role, "eng", Access::Allow)];
    let far_rules = vec![rule(RuleType::Role, "eng", Access::Deny)];
    let parents = [
        ResolveParent {
            entity: &near,
            rules: &near_rules,
            default_included: Some(false),
        },
        ResolveParent {
            entity: &far,
            rules: &far_rules,
            default_included: Some(false),
        },
    ];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &parents,
    });

    assert_eq!(
        decision,
        Decision::Allow {
            matched_by: MatchedBy::RoleAllow { role: "eng".into() }
        }
    );
}

#[test]
fn own_default_included_allows_before_parent_default() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &[],
        default_included: Some(false),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: Some(true),
        parents: &parents,
    });

    assert_eq!(
        decision,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    );
}

#[test]
fn parent_default_included_allows_when_own_default_off() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &[],
        default_included: Some(true),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: Some(false),
        parents: &parents,
    });

    assert_eq!(
        decision,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    );
}

#[test]
fn unknown_entity_when_nothing_matches_and_own_default_none() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &[],
        default_included: Some(false),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: None,
        parents: &parents,
    });

    assert!(
        matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::UnknownEntity { ref entity }
            } if entity.id_str() == "child-skill"
        ),
        "got {decision:?}",
    );
}

#[test]
fn parent_default_allows_even_when_own_default_none() {
    let child = child();
    let parent = parent_entity();
    let user = fixture_user_id();
    let roles: Vec<String> = vec!["eng".into()];
    let parents = [ResolveParent {
        entity: &parent,
        rules: &[],
        default_included: Some(true),
    }];

    let decision = resolve(ResolveInput {
        entity: &child,
        rules: &[],
        user_id: &user,
        user_roles: &roles,
        default_included: None,
        parents: &parents,
    });

    assert_eq!(
        decision,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    );
}
