//! The session pathway's gate: off by default, on only when the profile
//! lists the `session` source and switches the session block on; bounded
//! texts never split a character.

use systemprompt_manifest::profile::{BrainCaptureProfile, CaptureSource};
use systemprompt_models::brain::SessionTurnClosed;

#[test]
fn session_capture_is_off_by_default() {
    assert!(!BrainCaptureProfile::default().session_capture_on());
}

#[test]
fn the_switch_alone_or_the_source_alone_keeps_capture_off() {
    let mut switched = BrainCaptureProfile::default();
    switched.session.enabled = true;
    assert!(!switched.session_capture_on());
    let mut sourced = BrainCaptureProfile::default();
    sourced.sources.push(CaptureSource::Session);
    assert!(!sourced.session_capture_on());
    sourced.session.enabled = true;
    assert!(sourced.session_capture_on());
}

#[test]
fn texts_are_bounded_on_character_boundaries() {
    assert_eq!(SessionTurnClosed::bounded("héllo wörld", 5), "héllo");
    assert_eq!(SessionTurnClosed::bounded("short", 4000), "short");
    assert_eq!(SessionTurnClosed::bounded("", 3), "");
}
