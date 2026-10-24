//! `services/access-control/rules.yaml`, core's one access-control dialect:
//! parsing, validation, the canonical value and its rendering back.

use std::collections::BTreeMap;

use systemprompt_governance::authz::rules_file::{
    BandSpec, CanonicalEntity, EntityTarget, declared_entities, parse_rules_doc, render_rules_doc,
};
use systemprompt_governance::authz::types::{Access, EntityKind};

const DOC: &str = r#"
entities:
  - entity: marketplace/commons
    default: open
    why: Baseline workspace for everyone.
    allow:
      role: [user]
  - entity: gateway_route/*
    default: open
    why: Routes are open; entitlement narrows by model.
    allow:
      role: [user]
  - entity: model/claude-opus-*
    why: Opus is for engineering.
    allow:
      group: [engineering]
      role: { values: [admin], why: Admins evaluate every model. }
    deny:
      project: [frozen]
  - entity: provider/vertex
    owner: bundle:kit
    why: The EU account.
    allow:
      role: [eu]
"#;

fn problem(yaml: &str) -> String {
    parse_rules_doc(yaml).expect_err("refused").to_string()
}

#[test]
fn a_well_formed_file_parses_with_globs_entitlements_and_owners() {
    let doc = parse_rules_doc(DOC).expect("parses");
    let entities = declared_entities(&doc).expect("entities");

    let keys: Vec<&str> = entities.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "gateway_route/*",
            "marketplace/commons",
            "model/claude-opus-*",
            "provider/vertex"
        ]
    );
    assert_eq!(
        entities["provider/vertex"].owner_bundle.as_deref(),
        Some("kit")
    );
    let opus = &entities["model/claude-opus-*"].canonical;
    assert!(!opus.default_included);
    let whys: BTreeMap<&str, &str> = opus
        .rules
        .iter()
        .map(|r| (r.rule_value.as_str(), r.why.as_str()))
        .collect();
    assert_eq!(whys["admin"], "Admins evaluate every model.");
    assert_eq!(whys["engineering"], "Opus is for engineering.");
    assert!(
        opus.rules
            .iter()
            .any(|r| r.rule_value == "frozen" && r.access == Access::Deny)
    );
}

#[test]
fn an_empty_file_declares_nothing() {
    assert!(parse_rules_doc("  \n").expect("empty").entities.is_empty());
}

#[test]
fn every_structural_problem_is_refused() {
    assert!(
        problem("entities:\n  - entity: plugin/x\n    why: ' '\n    allow: {role: [a]}\n")
            .contains("`why`")
    );
    assert!(
        problem("entities:\n  - entity: plugin/x*\n    why: w\n    allow: {role: [a]}\n")
            .contains("glob")
    );
    assert!(problem("entities:\n  - entity: plugin/x\n    why: w\n").contains("no allow"));
    assert!(
        problem("entities:\n  - entity: plugin/x\n    why: w\n    allow: {user: [u1]}\n")
            .contains("not a band")
    );
    assert!(
        problem("entities:\n  - entity: plugin/x\n    why: w\n    allow: {role: []}\n")
            .contains("no subjects")
    );
    assert!(problem("entities:\n  - entity: plugin/x\n    why: w\n    allow: {role: [a]}\n    deny: {role: [a]}\n").contains("both allowed and denied"));
    assert!(
        problem(
            "entities:\n  - entity: plugin/x\n    owner: kit\n    why: w\n    allow: {role: [a]}\n"
        )
        .contains("bundle:")
    );
    assert!(problem("entities:\n  - entity: gateway_route/*\n    owner: bundle:k\n    why: w\n    allow: {role: [a]}\n").contains("cannot name an owner"));
    assert!(
        problem("entities:\n  - entity: nothing/x\n    why: w\n    allow: {role: [a]}\n")
            .contains("nothing")
    );
    assert!(problem("entities:\n  - entity: plugin/x\n    why: w\n    allow: {role: [a]}\n  - entity: plugin/x\n    why: w\n    allow: {role: [b]}\n").contains("more than once"));
    assert!(problem("rules: []\n").contains("rules"));
}

#[test]
fn the_canonical_value_ignores_band_grouping_and_order() {
    let a = parse_rules_doc(
        "entities:\n  - entity: plugin/x\n    why: w\n    allow:\n      role: [b, a]\n      group: [g]\n",
    )
    .expect("a");
    let b = parse_rules_doc(
        "entities:\n  - entity: plugin/x\n    why: w\n    allow:\n      group: [g]\n      role: [a, b]\n",
    )
    .expect("b");
    let value = |doc| {
        declared_entities(&doc).expect("entities")["plugin/x"]
            .canonical
            .to_value()
            .expect("json")
    };
    assert_eq!(value(a), value(b));
}

#[test]
fn rendering_round_trips_to_the_same_canonical_values() {
    let doc = parse_rules_doc(DOC).expect("parses");
    let canonical: BTreeMap<String, CanonicalEntity> = declared_entities(&doc)
        .expect("entities")
        .into_iter()
        .map(|(k, v)| (k, v.canonical))
        .collect();

    let text = render_rules_doc(&canonical, &BTreeMap::new()).expect("render");
    let back = parse_rules_doc(&text).expect("rendered file parses");
    let again: BTreeMap<String, CanonicalEntity> = declared_entities(&back)
        .expect("entities")
        .into_iter()
        .map(|(k, v)| (k, v.canonical))
        .collect();

    assert_eq!(canonical, again);
    let opus = back
        .entities
        .iter()
        .find(|d| d.entity == "model/claude-opus-*")
        .expect("opus");
    assert_eq!(opus.why, "Opus is for engineering.");
    assert!(matches!(
        opus.allow.get("role"),
        Some(BandSpec::Detailed { .. })
    ));
}

#[test]
fn targets_parse_kind_and_id() {
    let target: EntityTarget = "model/claude-*".parse().expect("target");
    assert_eq!(target.kind, EntityKind::Model);
    assert!(target.is_glob());
    assert!("provider".parse::<EntityTarget>().is_err());
    assert!("provider/ padded".parse::<EntityTarget>().is_err());
}
