use systemprompt_provider_contracts::{
    ApplyMode, ChangeActor, DeclaredState, DriftKind, EntityScope, PlaneEffect, PlaneSnapshot,
    diff_snapshots,
};

fn snapshot(pairs: &[(&str, &str)]) -> PlaneSnapshot {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn identical_snapshots_have_no_drift() {
    let s = snapshot(&[("a", "1"), ("b", "2")]);
    assert!(diff_snapshots(&s, &s).is_empty());
}

#[test]
fn drift_classifies_missing_changed_and_extra_with_both_values() {
    let declared = snapshot(&[("a", "1"), ("b", "2")]);
    let current = snapshot(&[("b", "3"), ("c", "4")]);
    let rows = diff_snapshots(&declared, &current);
    assert_eq!(rows.len(), 3);
    let a = rows.iter().find(|r| r.entity == "a").expect("a");
    assert_eq!(a.kind, DriftKind::Missing);
    assert_eq!(
        (a.declared.as_deref(), a.current.as_deref()),
        (Some("1"), None)
    );
    let b = rows.iter().find(|r| r.entity == "b").expect("b");
    assert_eq!(b.kind, DriftKind::Changed);
    assert_eq!(
        (b.declared.as_deref(), b.current.as_deref()),
        (Some("2"), Some("3"))
    );
    let c = rows.iter().find(|r| r.entity == "c").expect("c");
    assert_eq!(c.kind, DriftKind::Extra);
    assert_eq!(
        (c.declared.as_deref(), c.current.as_deref()),
        (None, Some("4"))
    );
}

#[test]
fn declared_hash_depends_on_content_not_insertion_order() {
    let mut forward = PlaneSnapshot::new();
    forward.insert("a".into(), "1".into());
    forward.insert("b".into(), "2".into());
    let mut backward = PlaneSnapshot::new();
    backward.insert("b".into(), "2".into());
    backward.insert("a".into(), "1".into());
    assert_eq!(
        DeclaredState::from_entries(forward).hash,
        DeclaredState::from_entries(backward.clone()).hash
    );
    let mut changed = backward;
    changed.insert("a".into(), "9".into());
    assert_ne!(
        DeclaredState::from_entries(changed).hash,
        DeclaredState::from_entries(snapshot(&[("a", "1"), ("b", "2")])).hash
    );
}

#[test]
fn declared_hash_separates_key_value_boundaries() {
    let joined = DeclaredState::from_entries(snapshot(&[("ab", "c")]));
    let split = DeclaredState::from_entries(snapshot(&[("a", "bc")]));
    assert_ne!(joined.hash, split.hash);
    assert_eq!(joined.hash.len(), 64);
}

#[test]
fn actor_labels_are_stable_tokens() {
    assert_eq!(ChangeActor::Boot.label(), "boot");
    assert_eq!(ChangeActor::Reconciler.label(), "config_reconcile");
    assert_eq!(ChangeActor::Cli.label(), "cli");
    assert_eq!(ChangeActor::Seed.label(), "seed");
}

#[test]
fn seed_if_empty_converges_like_overwrite_and_insert_only_does_not() {
    assert_eq!(ApplyMode::SeedIfEmpty.as_str(), "seed_if_empty");
    assert!(ApplyMode::SeedIfEmpty.must_converge());
    assert!(ApplyMode::Overwrite.must_converge());
    assert!(!ApplyMode::InsertOnly.must_converge());
}

#[test]
fn an_entity_scope_admits_only_the_named_keys() {
    let keys = vec!["groups/a".to_owned()];
    let scoped = EntityScope::Entities(&keys);
    assert!(scoped.admits("groups/a"));
    assert!(!scoped.admits("groups/b"));
    assert!(!scoped.is_all());
    assert!(EntityScope::All.admits("anything"));
}

#[test]
fn plane_effect_defaults_to_live() {
    assert_eq!(PlaneEffect::default(), PlaneEffect::Live);
    assert_eq!(PlaneEffect::OnRestart.as_str(), "on_restart");
}

mod resolve {
    use systemprompt_provider_contracts::{
        ApplyMode, ApplyRequest, ChangeActor, ChangeRequest, ChangeSource, DeclaredState,
        EntityScope, resolve_apply,
    };

    use super::snapshot;

    fn run(mode: ApplyMode, scope: EntityScope<'_>) -> Vec<(String, Option<String>)> {
        let declared = DeclaredState::from_entries(snapshot(&[("a", "1"), ("b", "2"), ("c", "3")]));
        let current = snapshot(&[("b", "2"), ("c", "old"), ("gone", "x")]);
        let change = ChangeRequest {
            actor: ChangeActor::Cli,
            reason: "test",
        };
        let request = ApplyRequest {
            declared: &declared,
            mode,
            change: &change,
            scope,
        };
        let resolved = resolve_apply(&request, &current, "planes/x.yaml");
        for entity in resolved.values() {
            assert_eq!(
                entity.source,
                ChangeSource::Code("planes/x.yaml".to_owned())
            );
        }
        resolved.into_iter().map(|(k, e)| (k, e.value)).collect()
    }

    fn pair(k: &str, v: Option<&str>) -> (String, Option<String>) {
        (k.to_owned(), v.map(str::to_owned))
    }

    #[test]
    fn overwrite_writes_differences_and_deletes_undeclared_rows() {
        assert_eq!(
            run(ApplyMode::Overwrite, EntityScope::All),
            vec![
                pair("a", Some("1")),
                pair("c", Some("3")),
                pair("gone", None)
            ]
        );
    }

    #[test]
    fn insert_only_adds_absent_entities_and_deletes_nothing() {
        assert_eq!(
            run(ApplyMode::InsertOnly, EntityScope::All),
            vec![pair("a", Some("1"))]
        );
    }

    #[test]
    fn a_scoped_apply_touches_only_the_named_entities() {
        let keys = ["c".to_owned(), "gone".to_owned()];
        assert_eq!(
            run(ApplyMode::SeedIfEmpty, EntityScope::Entities(&keys)),
            vec![pair("c", Some("3")), pair("gone", None)]
        );
    }
}
