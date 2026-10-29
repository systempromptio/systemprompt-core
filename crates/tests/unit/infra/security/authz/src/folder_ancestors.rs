use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_governance::authz::resolver::ResolveParent;
use systemprompt_governance::authz::types::{Access, AccessRule, Decision, RuleType};
use systemprompt_governance::authz::{
    ChainSources, DecisionFacts, EntityKind, EntityRef, NO_SUBJECT_ATTRIBUTES, ParentChainIndex,
    ResolveBase,
};
use systemprompt_identifiers::{DocumentFolderId, RuleId};
use systemprompt_test_fixtures::fixture_user_id;

fn role(value: &str, access: Access) -> AccessRule {
    AccessRule {
        id: RuleId::new(format!("{value}-{access}")),
        rule_type: RuleType::ROLE,
        rule_value: value.into(),
        access,
        justification: None,
    }
}

fn index() -> ParentChainIndex {
    ParentChainIndex::from_parts(
        BTreeMap::new(),
        BTreeMap::new(),
        Arc::new(ChainSources::default()),
    )
}

fn decide(
    own_rules: &[AccessRule],
    own_default: bool,
    ancestors: &[ResolveParent<'_>],
    roles: &[&str],
) -> Decision {
    let user = fixture_user_id();
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    index().resolve(
        EntityKind::Folder,
        DocumentFolderId::generate().as_str(),
        ResolveBase {
            rules: own_rules,
            user_id: &user,
            user_roles: &roles,
            default_included: Some(own_default),
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
            facts: DecisionFacts {
                model: None,
                provider: None,
                folder_ancestors: ancestors,
            },
        },
    )
}

fn folder() -> EntityRef {
    EntityRef::Folder(DocumentFolderId::generate())
}

#[test]
fn a_closed_folder_is_not_reopened_by_its_open_parent() {
    let parent = folder();
    let ancestors = [ResolveParent {
        entity: &parent,
        rules: &[],
        default_included: Some(true),
    }];

    let decision = decide(&[], false, &ancestors, &["user"]);
    assert!(!decision.permits(), "{decision:?}");
}

#[test]
fn an_open_folder_under_an_open_parent_is_admitted() {
    let parent = folder();
    let ancestors = [ResolveParent {
        entity: &parent,
        rules: &[],
        default_included: Some(true),
    }];

    assert!(decide(&[], true, &ancestors, &["user"]).permits());
}

#[test]
fn an_ancestor_deny_beats_a_nearer_folder_allow() {
    let near = folder();
    let far = folder();
    let near_rules = [role("user", Access::Allow)];
    let far_rules = [role("user", Access::Deny)];
    let ancestors = [
        ResolveParent {
            entity: &near,
            rules: &near_rules,
            default_included: Some(true),
        },
        ResolveParent {
            entity: &far,
            rules: &far_rules,
            default_included: Some(true),
        },
    ];

    let decision = decide(&[], true, &ancestors, &["user"]);
    assert!(matches!(decision, Decision::Deny { .. }), "{decision:?}");
}

#[test]
fn an_ancestor_deny_beats_the_folders_own_allow() {
    let far = folder();
    let far_rules = [role("contractor", Access::Deny)];
    let ancestors = [ResolveParent {
        entity: &far,
        rules: &far_rules,
        default_included: Some(true),
    }];
    let own = [role("contractor", Access::Allow)];

    assert!(!decide(&own, true, &ancestors, &["contractor"]).permits());
    assert!(
        !decide(&own, false, &ancestors, &["user"]).permits(),
        "a subject the ancestor does not deny still needs its own admission"
    );
}

#[test]
fn an_ancestor_allow_admits_a_ruleless_closed_folder() {
    let far = folder();
    let far_rules = [role("eng", Access::Allow)];
    let ancestors = [ResolveParent {
        entity: &far,
        rules: &far_rules,
        default_included: Some(false),
    }];

    assert!(decide(&[], false, &ancestors, &["eng"]).permits());
    assert!(!decide(&[], false, &ancestors, &["user"]).permits());
}
