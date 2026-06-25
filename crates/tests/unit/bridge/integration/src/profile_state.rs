use std::collections::BTreeMap;

use systemprompt_bridge::integration::ProfileState;

fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn empty_keys_means_absent() {
    let s = ProfileState::classify(&["a"], &BTreeMap::new(), None);
    assert!(matches!(s, ProfileState::Absent));
}

#[test]
fn all_required_present_means_installed() {
    let s = ProfileState::classify(
        &["a", "b"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
        None,
    );
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn missing_required_key_means_partial() {
    let s = ProfileState::classify(
        &["a", "b", "c"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
        None,
    );
    match s {
        ProfileState::Partial { missing_required } => {
            assert_eq!(missing_required, vec!["c".to_string()]);
        },
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn matching_secret_keeps_installed() {
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), Some(true));
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn mismatched_secret_downgrades_installed_to_stale() {
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), Some(false));
    assert!(matches!(s, ProfileState::Stale));
}

#[test]
fn unknown_secret_never_downgrades() {
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), None);
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn mismatched_secret_does_not_promote_partial_to_stale() {
    let s = ProfileState::classify(&["a", "b"], &keys(&[("a", "1")]), Some(false));
    assert!(matches!(s, ProfileState::Partial { .. }));
}
