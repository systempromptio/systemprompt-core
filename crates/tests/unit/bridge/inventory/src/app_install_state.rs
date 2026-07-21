//! `AppInstallState` semantics and wire format.
//!
//! The web UI branches on the serialized *strings* (see `chooseBadge` in
//! `web/js/components/sp-host-card.js`), so the representation is part of the
//! contract, not an implementation detail.

use systemprompt_bridge::integration::host_app::AppInstallState;

#[test]
fn only_installed_counts_as_installed() {
    assert!(AppInstallState::Installed.is_installed());
    assert!(!AppInstallState::NotInstalled.is_installed());
    // The whole point of the tri-state: an inconclusive probe is not evidence
    // the app is present either.
    assert!(!AppInstallState::Unknown.is_installed());
}

#[test]
fn unknown_is_the_only_inconclusive_state() {
    assert!(AppInstallState::Installed.is_conclusive());
    assert!(AppInstallState::NotInstalled.is_conclusive());
    assert!(!AppInstallState::Unknown.is_conclusive());
}

#[test]
fn serializes_to_the_tags_the_ui_matches_on() {
    let tag = |s: AppInstallState| serde_json::to_string(&s).expect("serialize");
    assert_eq!(tag(AppInstallState::Installed), "\"installed\"");
    assert_eq!(tag(AppInstallState::NotInstalled), "\"not_installed\"");
    assert_eq!(tag(AppInstallState::Unknown), "\"unknown\"");
}
