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
    let s = ProfileState::from_keys(&["a"], &BTreeMap::new());
    assert!(matches!(s, ProfileState::Absent));
}

#[test]
fn all_required_present_means_installed() {
    let s = ProfileState::from_keys(
        &["a", "b"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
    );
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn missing_required_key_means_partial() {
    let s = ProfileState::from_keys(
        &["a", "b", "c"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
    );
    match s {
        ProfileState::Partial { missing_required } => {
            assert_eq!(missing_required, vec!["c".to_string()]);
        },
        other => panic!("expected Partial, got {other:?}"),
    }
}
