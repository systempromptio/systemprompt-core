//! Decision-time entitlement (credential.md §3.1 rule 7): `model/<glob>`,
//! `provider/<name>` and route globs are matched against the request when it
//! is decided, never expanded when they are applied.

use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_governance::authz::types::{Access, AccessRule, Decision, RuleType};
use systemprompt_governance::authz::{
    ChainSources, DecisionFacts, EntityKind, EntityRef, LoadedParent, NO_SUBJECT_ATTRIBUTES,
    ParentChainIndex, ResolveBase,
};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, RuleId};
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

fn level(entity: EntityRef, rules: Vec<AccessRule>, open: bool) -> LoadedParent {
    LoadedParent {
        entity,
        rules,
        default_included: Some(open),
    }
}

fn index(patterns: Vec<LoadedParent>) -> ParentChainIndex {
    ParentChainIndex::from_parts(
        BTreeMap::new(),
        BTreeMap::new(),
        Arc::new(ChainSources::default()),
    )
    .with_patterns(patterns)
}

fn decide(
    index: &ParentChainIndex,
    route_rules: &[AccessRule],
    roles: &[&str],
    model: &str,
) -> Decision {
    let user = fixture_user_id();
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    index.resolve(
        EntityKind::GatewayRoute,
        "route-a",
        ResolveBase {
            rules: route_rules,
            user_id: &user,
            user_roles: &roles,
            default_included: Some(false),
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
            facts: DecisionFacts {
                model: Some(model),
                provider: Some("vertex"),
            },
        },
    )
}

fn blanket_routes() -> LoadedParent {
    level(
        EntityRef::GatewayRoute(RouteId::new("*")),
        vec![role("user", Access::Allow)],
        true,
    )
}

#[test]
fn a_model_glob_narrows_a_blanket_route_grant() {
    let idx = index(vec![
        blanket_routes(),
        level(
            EntityRef::Model(ModelId::new("claude-opus-*")),
            vec![role("eng", Access::Allow)],
            false,
        ),
    ]);

    assert!(!decide(&idx, &[], &["user"], "claude-opus-5").permits());
    assert!(decide(&idx, &[], &["user", "eng"], "claude-opus-5").permits());
    assert!(
        decide(&idx, &[], &["user"], "claude-sonnet-5").permits(),
        "a model no entitlement names falls through to the route glob"
    );
}

#[test]
fn an_exact_model_entity_is_consulted_before_a_glob() {
    let idx = index(vec![
        level(
            EntityRef::Model(ModelId::new("claude-*")),
            vec![role("eng", Access::Allow)],
            false,
        ),
        level(
            EntityRef::Model(ModelId::new("claude-haiku-5")),
            vec![role("user", Access::Allow)],
            false,
        ),
    ]);

    assert!(decide(&idx, &[], &["user"], "claude-haiku-5").permits());
    assert!(!decide(&idx, &[], &["user"], "claude-opus-5").permits());
}

#[test]
fn a_provider_deny_refuses_the_request_whatever_the_route_glob_allows() {
    let idx = index(vec![
        blanket_routes(),
        level(
            EntityRef::Provider(ProviderId::new("vertex")),
            vec![role("contractor", Access::Deny)],
            true,
        ),
    ]);

    let decision = decide(&idx, &[], &["user", "contractor"], "gemini-3");
    assert!(matches!(decision, Decision::Deny { .. }), "{decision:?}");
    assert!(decide(&idx, &[], &["user"], "gemini-3").permits());
}

#[test]
fn the_routes_own_rules_decide_before_any_entitlement() {
    let idx = index(vec![level(
        EntityRef::Model(ModelId::new("claude-opus-*")),
        vec![role("eng", Access::Allow)],
        false,
    )]);

    let route_rules = [role("user", Access::Allow)];
    assert!(decide(&idx, &route_rules, &["user"], "claude-opus-5").permits());
}

#[test]
fn a_decision_without_facts_ignores_entitlements() {
    let idx = index(vec![level(
        EntityRef::Model(ModelId::new("*")),
        vec![role("eng", Access::Allow)],
        false,
    )]);
    let user = fixture_user_id();
    let roles = vec!["user".to_owned()];

    let parents = idx.pattern_parents(
        EntityKind::GatewayRoute,
        "route-a",
        DecisionFacts::default(),
    );
    assert!(parents.is_empty());
    let decision = idx.resolve(
        EntityKind::GatewayRoute,
        "route-a",
        ResolveBase {
            rules: &[],
            user_id: &user,
            user_roles: &roles,
            default_included: Some(true),
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
            facts: DecisionFacts::default(),
        },
    );
    assert!(decision.permits());
}

fn decide_model(index: &ParentChainIndex, roles: &[&str], model: &str) -> Decision {
    let user = fixture_user_id();
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    index.resolve(
        EntityKind::Model,
        model,
        ResolveBase {
            rules: &[],
            user_id: &user,
            user_roles: &roles,
            default_included: None,
            attributes: &NO_SUBJECT_ATTRIBUTES,
            dimensions: &[],
            facts: DecisionFacts {
                model: Some(model),
                provider: Some("vertex"),
            },
        },
    )
}

#[test]
fn a_request_no_route_matched_is_decided_on_its_model_then_its_provider() {
    let idx = index(vec![
        level(
            EntityRef::Model(ModelId::new("claude-opus-*")),
            vec![role("eng", Access::Allow)],
            false,
        ),
        level(
            EntityRef::Provider(ProviderId::new("vertex")),
            vec![role("user", Access::Allow)],
            false,
        ),
    ]);

    assert!(!decide_model(&idx, &["user"], "claude-opus-5").permits());
    assert!(decide_model(&idx, &["eng"], "claude-opus-5").permits());
    assert!(
        decide_model(&idx, &["user"], "claude-sonnet-5").permits(),
        "a model no entitlement names falls through to its provider"
    );
    assert!(
        !decide_model(&index(Vec::new()), &["user"], "claude-sonnet-5").permits(),
        "with nothing declared the request fails closed"
    );
}
