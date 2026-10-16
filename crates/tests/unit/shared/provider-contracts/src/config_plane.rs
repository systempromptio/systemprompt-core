use systemprompt_provider_contracts::{
    ChangeActor, DeclaredState, DriftKind, PlaneSnapshot, diff_snapshots,
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
}
