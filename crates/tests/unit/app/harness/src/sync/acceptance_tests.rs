use chrono::TimeDelta;
use serde_json::json;
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::manifest_version::ManifestVersion;

use systemprompt_harness::sync::acceptance::{Refusal, accept};

use crate::sync::fakes::{at, manifest, manifest_body, version};

fn parsed(seconds: i64, suffix: &str) -> ManifestVersion {
    ManifestVersion::try_new(version(seconds, suffix)).unwrap()
}

#[test]
fn a_fresh_manifest_of_the_user_is_accepted() {
    let accepted = accept(&manifest(json!({})), Some(&UserId::new("u-1")), None, at(0)).unwrap();
    assert_eq!(accepted, parsed(0, "0000000000000001"));
}

#[test]
fn another_users_manifest_is_refused() {
    let refused = accept(&manifest(json!({})), Some(&UserId::new("u-2")), None, at(0));
    assert_eq!(refused, Err(Refusal::User));
}

#[test]
fn a_manifest_older_than_the_last_applied_is_refused_and_the_same_one_is_accepted_again() {
    let last = parsed(0, "0000000000000001");
    assert_eq!(
        accept(&manifest(json!({})), None, Some(&last), at(0)),
        Ok(last.clone())
    );
    let older = manifest(json!({ "manifest_version": version(-10, "ffffffffffffffff") }));
    assert_eq!(
        accept(&older, None, Some(&last), at(0)),
        Err(Refusal::Replay)
    );
    let newer = manifest(json!({ "manifest_version": version(0, "0000000000000002") }));
    assert!(accept(&newer, None, Some(&last), at(0)).is_ok());
}

#[test]
fn an_older_manifest_offered_after_one_was_applied_is_refused_as_a_replay() {
    let applied = accept(&manifest(json!({})), None, None, at(0)).unwrap();
    let older = manifest(json!({ "manifest_version": version(-86_400, "0000000000000001") }));
    assert_eq!(
        accept(&older, None, Some(&applied), at(0)),
        Err(Refusal::Replay)
    );
}

#[test]
fn not_before_beyond_five_minutes_is_refused_either_way_before_anything_was_applied() {
    let ahead = manifest(json!({ "not_before": (at(0) + TimeDelta::minutes(6)).to_rfc3339() }));
    assert_eq!(accept(&ahead, None, None, at(0)), Err(Refusal::Skew));
    let behind = manifest(json!({ "not_before": (at(0) - TimeDelta::minutes(6)).to_rfc3339() }));
    assert_eq!(accept(&behind, None, None, at(0)), Err(Refusal::Skew));
    let close = manifest(json!({ "not_before": (at(0) - TimeDelta::minutes(4)).to_rfc3339() }));
    assert!(accept(&close, None, None, at(0)).is_ok());
    let last = parsed(-3600, "0000000000000001");
    assert!(accept(&behind, None, Some(&last), at(0)).is_ok());
    assert_eq!(accept(&ahead, None, Some(&last), at(0)), Err(Refusal::Skew));
}

#[test]
fn a_manifest_without_version_or_not_before_is_refused() {
    for field in ["manifest_version", "not_before"] {
        let mut absent = manifest_body(json!({}));
        absent.as_object_mut().unwrap().remove(field);
        for body in [manifest_body(json!({ field: null })), absent] {
            let refused = Manifest::parse(&body.to_string());
            assert!(refused.is_err(), "{field}: {refused:?}");
        }
    }
}
