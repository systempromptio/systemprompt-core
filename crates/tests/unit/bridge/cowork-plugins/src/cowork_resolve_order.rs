// The Cowork session order: an explicit local pin beats the organization the
// gateway's profile names, which beats the personal or only session; several
// usable sessions with none of those is an error naming the fix.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use systemprompt_bridge::cowork_compat::PERSONAL_SESSION_UUID;
use systemprompt_bridge::integration::cowork_plugins::{
    ResolveTargetError, TargetSource, choose_target,
};

const GATEWAY_ORG: &str = "f8e4d915-1111-2222-3333-444444444444";
const OTHER_ORG: &str = "a1b2c3d4-5555-6666-7777-888888888888";

struct Sessions(tempfile::TempDir);

impl Sessions {
    fn new() -> Self {
        Self(tempfile::tempdir().expect("tempdir"))
    }

    fn add(&self, org: &str) -> PathBuf {
        let path = self.0.path().join("acct").join(org);
        fs::create_dir_all(path.join("cowork_plugins")).expect("mkdir");
        path
    }
}

fn at(path: &PathBuf, secs: u64) -> (SystemTime, PathBuf) {
    (
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
        path.clone(),
    )
}

#[test]
fn pin_beats_the_gateway_org_and_the_personal_session() {
    let s = Sessions::new();
    let personal = s.add(PERSONAL_SESSION_UUID);
    let gateway = s.add(GATEWAY_ORG);
    let pinned = s.add(OTHER_ORG);
    let candidates = vec![at(&personal, 1), at(&gateway, 2), at(&pinned, 3)];

    let chosen = choose_target(Some(pinned.clone()), Some(GATEWAY_ORG), &candidates).unwrap();
    assert_eq!(chosen, Some((pinned, TargetSource::Pinned)));
}

#[test]
fn gateway_org_beats_the_personal_session() {
    let s = Sessions::new();
    let personal = s.add(PERSONAL_SESSION_UUID);
    let gateway = s.add(GATEWAY_ORG);
    let candidates = vec![at(&personal, 1), at(&gateway, 2)];

    let chosen = choose_target(None, Some(GATEWAY_ORG), &candidates).unwrap();
    assert_eq!(chosen, Some((gateway, TargetSource::Gateway)));
}

#[test]
fn gateway_org_matches_an_unhyphenated_directory_name() {
    let s = Sessions::new();
    let gateway = s.add(&GATEWAY_ORG.replace('-', ""));
    let other = s.add(OTHER_ORG);
    let candidates = vec![at(&gateway, 1), at(&other, 2)];

    let chosen = choose_target(None, Some(GATEWAY_ORG), &candidates).unwrap();
    assert_eq!(chosen, Some((gateway, TargetSource::Gateway)));
}

#[test]
fn personal_session_wins_when_the_gateway_names_none_present() {
    let s = Sessions::new();
    let personal = s.add(PERSONAL_SESSION_UUID);
    let other = s.add(OTHER_ORG);
    let candidates = vec![at(&personal, 1), at(&other, 2)];

    let chosen = choose_target(None, Some(GATEWAY_ORG), &candidates).unwrap();
    assert_eq!(chosen, Some((personal, TargetSource::Personal)));
}

#[test]
fn only_session_is_chosen_without_an_org() {
    let s = Sessions::new();
    let only = s.add(OTHER_ORG);

    let chosen = choose_target(None, None, &[at(&only, 1)]).unwrap();
    assert_eq!(chosen, Some((only, TargetSource::OnlySession)));
}

#[test]
fn several_sessions_with_no_rule_are_ambiguous_and_name_the_fix() {
    let s = Sessions::new();
    let a = s.add(GATEWAY_ORG);
    let b = s.add(OTHER_ORG);

    let err = choose_target(None, None, &[at(&a, 1), at(&b, 2)]).unwrap_err();
    let ResolveTargetError::Ambiguous { candidates } = &err else {
        panic!("expected Ambiguous, got {err}");
    };
    assert_eq!(candidates.len(), 2);
    let message = err.to_string();
    assert!(message.contains("cowork.session_org_dir"), "{message}");
    assert!(message.contains("Choose session"), "{message}");
}
