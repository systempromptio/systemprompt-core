use std::collections::BTreeMap;

use systemprompt_bridge::integration::host_app::{AppInstallState, ProfileState, StaleReason};
use systemprompt_bridge::integration::profile_state::ProfileCode;
use systemprompt_bridge::verdict::Tone;

fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn only_a_complete_fresh_profile_reports_installed() {
    assert!(ProfileState::Installed.is_installed());
    assert!(!ProfileState::Absent.is_installed());
    assert!(
        !ProfileState::Stale {
            reason: StaleReason::ProxyPort
        }
        .is_installed(),
        "a stale profile is complete but unusable, so it is not installed"
    );
    assert!(
        !ProfileState::Partial {
            missing_required: vec!["a".to_owned()]
        }
        .is_installed()
    );
}

#[test]
fn every_profile_state_maps_to_its_own_gui_code() {
    assert_eq!(ProfileState::Absent.code(), ProfileCode::Absent);
    assert_eq!(ProfileState::Installed.code(), ProfileCode::Installed);
    assert_eq!(
        ProfileState::Partial {
            missing_required: Vec::new()
        }
        .code(),
        ProfileCode::Partial
    );
    assert_eq!(
        ProfileState::Stale {
            reason: StaleReason::LoopbackSecret
        }
        .code(),
        ProfileCode::Stale
    );
}

#[test]
fn a_repairable_profile_is_amber_and_a_missing_one_is_red() {
    assert_eq!(ProfileState::Installed.tone(), Tone::Ok);
    assert_eq!(
        ProfileState::Stale {
            reason: StaleReason::ProxyPort
        }
        .tone(),
        Tone::Warn,
        "stale is repairable, so it must not read as a failure"
    );
    assert_eq!(
        ProfileState::Partial {
            missing_required: vec!["a".to_owned()]
        }
        .tone(),
        Tone::Warn
    );
    assert_eq!(ProfileState::Absent.tone(), Tone::Err);
}

#[test]
fn the_verdict_pairs_the_tone_with_the_same_code_the_wire_carries() {
    let verdict = ProfileState::Stale {
        reason: StaleReason::LoopbackSecret,
    }
    .verdict();
    assert_eq!(verdict.tone, Tone::Warn);
    assert_eq!(verdict.code, ProfileCode::Stale);

    let absent = ProfileState::Absent.verdict();
    assert_eq!(absent.tone, Tone::Err);
    assert_eq!(absent.code, ProfileCode::Absent);
}

#[test]
fn missing_required_keys_are_listed_only_for_a_partial_profile() {
    let partial = ProfileState::classify(&["a", "b"], &keys(&[("a", "1")]), None, None);
    assert_eq!(partial.missing_required(), ["b".to_owned()]);
    assert!(ProfileState::Installed.missing_required().is_empty());
    assert!(ProfileState::Absent.missing_required().is_empty());
    assert!(
        ProfileState::Stale {
            reason: StaleReason::ProxyPort
        }
        .missing_required()
        .is_empty(),
        "a stale profile has every key; nothing is missing"
    );
}

#[test]
fn a_loopback_url_on_the_live_port_is_fresh_and_one_on_another_port_is_not() {
    assert_eq!(
        ProfileState::endpoint_freshness(Some("http://127.0.0.1:48217/v1"), 48217),
        Some(true)
    );
    assert_eq!(
        ProfileState::endpoint_freshness(Some("http://127.0.0.1:48217/v1"), 51999),
        Some(false),
        "a profile baked for the old port is not fresh"
    );
}

#[test]
fn an_inconclusive_app_probe_is_amber_and_never_reads_as_not_installed() {
    assert!(AppInstallState::Installed.is_installed());
    assert!(!AppInstallState::Unknown.is_installed());
    assert!(!AppInstallState::NotInstalled.is_installed());

    assert_eq!(AppInstallState::Installed.tone(), Tone::Ok);
    assert_eq!(
        AppInstallState::Unknown.tone(),
        Tone::Warn,
        "an inconclusive probe must not be rendered as a red 'not installed'"
    );
    assert_eq!(AppInstallState::NotInstalled.tone(), Tone::Err);

    let verdict = AppInstallState::Unknown.verdict();
    assert_eq!(verdict.tone, Tone::Warn);
    assert_eq!(verdict.code, AppInstallState::Unknown);
}
