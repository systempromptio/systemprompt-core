use std::collections::BTreeMap;

use systemprompt_bridge::integration::{Freshness, ProfileProbe, ProfileState, StaleReason};

fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn classify(
    required: &[&str],
    present: &BTreeMap<String, String>,
    secret: Freshness,
    endpoint: Freshness,
) -> ProfileState {
    ProfileState::classify(&ProfileProbe {
        required,
        present,
        read_error: None,
        secret,
        endpoint,
    })
}

#[test]
fn empty_keys_means_absent() {
    let s = classify(
        &["a"],
        &BTreeMap::new(),
        Freshness::Unchecked,
        Freshness::Unchecked,
    );
    assert!(matches!(s, ProfileState::Absent));
}

#[test]
fn all_required_present_means_installed() {
    let s = classify(
        &["a", "b"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
        Freshness::Unchecked,
        Freshness::Unchecked,
    );
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn missing_required_key_means_partial() {
    let s = classify(
        &["a", "b", "c"],
        &keys(&[("a", "1"), ("b", "2"), ("extra", "x")]),
        Freshness::Unchecked,
        Freshness::Unchecked,
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
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Fresh,
        Freshness::Unchecked,
    );
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn mismatched_secret_downgrades_installed_to_stale() {
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Stale,
        Freshness::Unchecked,
    );
    assert!(matches!(
        s,
        ProfileState::Stale {
            reason: StaleReason::LoopbackSecret
        }
    ));
}

#[test]
fn an_unchecked_secret_never_downgrades() {
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Unchecked,
        Freshness::Unchecked,
    );
    assert!(matches!(s, ProfileState::Installed));
}

// A guard that cannot evaluate never reports green: a secret the host carries
// but the probe could not read is reported as unverifiable, not installed.
#[test]
fn an_unverifiable_secret_is_reported_not_assumed_fresh() {
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Unverifiable {
            reason: "the live secret could not be read".to_owned(),
        },
        Freshness::Unchecked,
    );
    assert!(
        matches!(&s, ProfileState::Unverifiable { reason } if reason.contains("could not be read")),
        "{s:?}"
    );
}

#[test]
fn mismatched_secret_does_not_promote_partial_to_stale() {
    let s = classify(
        &["a", "b"],
        &keys(&[("a", "1")]),
        Freshness::Stale,
        Freshness::Unchecked,
    );
    assert!(matches!(s, ProfileState::Partial { .. }));
}

#[test]
fn a_profile_baked_for_the_wrong_port_is_stale_not_installed() {
    // The WSL2/Windows case: the proxy moved off the default port, so a profile
    // written for the old one 403s on every request. Reporting it as Installed
    // is what let the GUI call a dead configuration healthy.
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Unchecked,
        Freshness::Stale,
    );
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
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Unchecked,
        Freshness::Fresh,
    );
    assert!(matches!(s, ProfileState::Installed));
}

#[test]
fn a_wrong_secret_is_named_ahead_of_a_wrong_port() {
    // Both are fixed by the same re-apply, so the more familiar diagnosis wins.
    let s = classify(
        &["a"],
        &keys(&[("a", "1")]),
        Freshness::Stale,
        Freshness::Stale,
    );
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

// An unreadable profile with nothing decoded is not "absent": absent would
// invite a fresh install over a file the bridge could not even read.
#[test]
fn a_read_error_with_no_keys_is_unverifiable_not_absent() {
    let s = ProfileState::classify(&ProfileProbe {
        required: &["a"],
        present: &BTreeMap::new(),
        read_error: Some("config.toml: permission denied"),
        secret: Freshness::Unchecked,
        endpoint: Freshness::Unchecked,
    });
    assert!(
        matches!(&s, ProfileState::Unverifiable { reason } if reason.contains("permission denied")),
        "{s:?}"
    );
}

#[test]
fn freshness_compare_distinguishes_unchecked_from_unverifiable() {
    assert_eq!(
        Freshness::compare(Some("x"), Some("x"), "secret"),
        Freshness::Fresh
    );
    assert_eq!(
        Freshness::compare(Some("x"), Some("y"), "secret"),
        Freshness::Stale
    );
    assert_eq!(
        Freshness::compare(None, Some("y"), "secret"),
        Freshness::Unchecked
    );
    assert!(matches!(
        Freshness::compare(Some("x"), None, "secret"),
        Freshness::Unverifiable { .. }
    ));
}

#[test]
fn endpoint_freshness_is_unchecked_for_an_absent_url_and_unverifiable_otherwise() {
    assert_eq!(
        ProfileState::endpoint_freshness(None, 48217),
        Freshness::Unchecked
    );
    assert_eq!(
        ProfileState::endpoint_freshness(Some(""), 48217),
        Freshness::Unchecked
    );
    assert!(matches!(
        ProfileState::endpoint_freshness(Some("https://gateway.example.com/v1"), 48217),
        Freshness::Unverifiable { .. }
    ));
    assert!(matches!(
        ProfileState::endpoint_freshness(Some("garbage"), 48217),
        Freshness::Unverifiable { .. }
    ));
    assert_eq!(
        ProfileState::endpoint_freshness(Some("http://127.0.0.1:48217"), 48217),
        Freshness::Fresh
    );
    assert_eq!(
        ProfileState::endpoint_freshness(Some("http://127.0.0.1:48218"), 48217),
        Freshness::Stale
    );
}
