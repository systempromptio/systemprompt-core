use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_bridge::drift::{DesiredProfile, EntryRole, diff};
use systemprompt_bridge::integration::host_app::{DesiredError, ProfileFactsError};
use systemprompt_bridge::integration::{ProfileProbe, ProfileState, StaleReason};

fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn desired() -> DesiredProfile {
    let mut d = DesiredProfile::new();
    d.push("api_key", "fp-live", EntryRole::Secret);
    d.push("base_url", "http://127.0.0.1:48217/v1", EntryRole::Endpoint);
    d.push("models", r#"["a","b"]"#, EntryRole::Models);
    d.push("servers", r#"[{"name":"x"}]"#, EntryRole::ManagedServers);
    d.push("telemetry", "false", EntryRole::Setting);
    d.push("headers", r#"{"a":"1","b":"2"}"#, EntryRole::Setting);
    d
}

fn current() -> BTreeMap<String, String> {
    keys(&[
        ("api_key", "fp-live"),
        ("base_url", "http://127.0.0.1:48217/v1"),
        ("models", r#"["a","b"]"#),
        ("servers", r#"[{"name":"x"}]"#),
        ("telemetry", "false"),
        ("headers", r#"{"a":"1","b":"2"}"#),
    ])
}

fn classify_with(
    observed: &BTreeMap<String, String>,
    desired: &Result<DesiredProfile, DesiredError>,
) -> ProfileState {
    ProfileState::classify(&ProfileProbe {
        present: observed,
        observed,
        read_error: None,
        desired,
    })
}

fn classify(observed: &BTreeMap<String, String>) -> ProfileState {
    classify_with(observed, &Ok(desired()))
}

fn with(changes: &[(&str, Option<&str>)]) -> BTreeMap<String, String> {
    let mut observed = current();
    for (key, value) in changes {
        match value {
            Some(v) => observed.insert((*key).to_owned(), (*v).to_owned()),
            None => observed.remove(*key),
        };
    }
    observed
}

fn stale(state: &ProfileState) -> (StaleReason, Vec<String>) {
    match state {
        ProfileState::Stale { reason, keys } => (*reason, keys.clone()),
        other => panic!("expected Stale, got {other:?}"),
    }
}

#[test]
fn nothing_read_means_absent() {
    assert!(matches!(classify(&BTreeMap::new()), ProfileState::Absent));
}

#[test]
fn every_desired_key_matching_means_installed_whatever_else_is_there() {
    let observed = with(&[("operator_key", Some("kept"))]);
    assert!(matches!(classify(&observed), ProfileState::Installed));
}

#[test]
fn json_equivalent_values_are_the_same_setting() {
    let observed = with(&[
        ("headers", Some(r#"{ "b": "2",  "a": "1" }"#)),
        ("models", Some(r#"[ "a", "b" ]"#)),
    ]);
    assert!(
        matches!(classify(&observed), ProfileState::Installed),
        "a re-serialised value is not drift"
    );
}

#[test]
fn a_missing_secret_or_endpoint_is_partial() {
    for key in ["api_key", "base_url"] {
        match classify(&with(&[(key, None)])) {
            ProfileState::Partial { missing_required } => {
                assert_eq!(missing_required, vec![key.to_owned()]);
            },
            other => panic!("{key}: expected Partial, got {other:?}"),
        }
    }
}

#[test]
fn a_missing_setting_is_stale_not_partial() {
    let (reason, keys) = stale(&classify(&with(&[("telemetry", None)])));
    assert_eq!(reason, StaleReason::Settings);
    assert_eq!(keys, vec!["telemetry".to_owned()]);
}

#[test]
fn a_differing_setting_is_stale_for_settings() {
    let (reason, keys) = stale(&classify(&with(&[("telemetry", Some("true"))])));
    assert_eq!(reason, StaleReason::Settings);
    assert_eq!(keys, vec!["telemetry".to_owned()]);
}

#[test]
fn a_differing_model_list_is_stale_for_the_model_list() {
    let (reason, keys) = stale(&classify(&with(&[("models", Some(r#"["a"]"#))])));
    assert_eq!(reason, StaleReason::ModelList);
    assert_eq!(keys, vec!["models".to_owned()]);
}

#[test]
fn a_differing_server_list_is_stale_for_the_server_list() {
    let (reason, _) = stale(&classify(&with(&[("servers", Some("[]"))])));
    assert_eq!(reason, StaleReason::ManagedServers);
}

#[test]
fn a_moved_port_is_stale_for_the_port() {
    let (reason, _) = stale(&classify(&with(&[(
        "base_url",
        Some("http://127.0.0.1:1/v1"),
    )])));
    assert_eq!(reason, StaleReason::ProxyPort);
    assert!(reason.breaks_routing());
}

#[test]
fn a_stale_secret_is_named_ahead_of_every_other_drift() {
    let (reason, keys) = stale(&classify(&with(&[
        ("api_key", Some("fp-old")),
        ("telemetry", Some("true")),
        ("base_url", Some("http://127.0.0.1:1/v1")),
    ])));
    assert_eq!(reason, StaleReason::LoopbackSecret);
    assert!(reason.breaks_routing());
    assert_eq!(keys.len(), 3, "every drifted key is listed: {keys:?}");
}

#[test]
fn settings_that_cannot_be_built_are_unverifiable_never_installed() {
    let unbuildable: Result<DesiredProfile, DesiredError> = Err(DesiredError::Facts(Arc::new(
        ProfileFactsError::LoopbackSecret(std::io::Error::other("secret unreadable")),
    )));
    assert!(
        matches!(
            classify_with(&current(), &unbuildable),
            ProfileState::Unverifiable { reason } if reason.contains("secret unreadable")
        ),
        "a guard that cannot evaluate never reports green"
    );
}

#[test]
fn a_read_error_with_no_keys_is_unverifiable_not_absent() {
    let empty = BTreeMap::new();
    let state = ProfileState::classify(&ProfileProbe {
        present: &empty,
        observed: &empty,
        read_error: Some("permission denied"),
        desired: &Ok(desired()),
    });
    assert!(
        matches!(&state, ProfileState::Unverifiable { reason } if reason == "permission denied"),
        "{state:?}"
    );
}

#[test]
fn the_shared_diff_separates_missing_from_differing() {
    let drift = diff(
        &desired(),
        &with(&[("telemetry", None), ("models", Some("[]"))]),
    );
    assert_eq!(drift.missing, vec!["telemetry".to_owned()]);
    assert_eq!(drift.differing, vec!["models".to_owned()]);
    assert!(!drift.is_clean());
    assert!(diff(&desired(), &current()).is_clean());
}
