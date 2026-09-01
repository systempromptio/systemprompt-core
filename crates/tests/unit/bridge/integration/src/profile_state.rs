use std::collections::BTreeMap;

use systemprompt_bridge::integration::{ProfileState, StaleReason};

fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn empty_keys_means_absent() {
    let s = ProfileState::classify(&["a"], &BTreeMap::new(), None, None);
    assert!(matches!(s, ProfileState::Absent));
}

#[test]
fn all_required_present_means_installed() {
    let s = ProfileState::classify(
        &["a", "b"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
        None,
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
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), Some(true), None);
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn mismatched_secret_downgrades_installed_to_stale() {
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), Some(false), None);
    assert!(matches!(
        s,
        ProfileState::Stale {
            reason: StaleReason::LoopbackSecret
        }
    ));
}

#[test]
fn unknown_secret_never_downgrades() {
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), None, None);
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn mismatched_secret_does_not_promote_partial_to_stale() {
    let s = ProfileState::classify(&["a", "b"], &keys(&[("a", "1")]), Some(false), None);
    assert!(matches!(s, ProfileState::Partial { .. }));
}

#[test]
fn a_profile_baked_for_the_wrong_port_is_stale_not_installed() {
    // The WSL2/Windows case: the proxy moved off the default port, so a profile
    // written for the old one 403s on every request. Reporting it as Installed
    // is what let the GUI call a dead configuration healthy.
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), None, Some(false));
    assert!(
        matches!(
            s,
            ProfileState::Stale {
                reason: StaleReason::ProxyPort
            }
        ),
        "{s:?}"
    );
}

#[test]
fn a_matching_port_keeps_installed() {
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), None, Some(true));
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn an_unknown_endpoint_never_downgrades() {
    // A deliberately remote gateway is not this check's business, and a false
    // "re-apply required" trains people to ignore the real one.
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), None, None);
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn a_wrong_secret_is_named_ahead_of_a_wrong_port() {
    // Both are fixed by the same re-apply, so the more familiar diagnosis wins.
    let s = ProfileState::classify(&["a"], &keys(&[("a", "1")]), Some(false), Some(false));
    assert!(
        matches!(
            s,
            ProfileState::Stale {
                reason: StaleReason::LoopbackSecret
            }
        ),
        "{s:?}"
    );
}

#[test]
fn endpoint_freshness_ignores_anything_that_is_not_a_loopback_url() {
    assert_eq!(ProfileState::endpoint_freshness(None, 48217), None);
    assert_eq!(ProfileState::endpoint_freshness(Some(""), 48217), None);
    assert_eq!(
        ProfileState::endpoint_freshness(Some("https://gateway.example.com/v1"), 48217),
        None
    );
    assert_eq!(
        ProfileState::endpoint_freshness(Some("garbage"), 48217),
        None
    );
}
