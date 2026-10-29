use systemprompt_bridge::config::gateways::GatewayProfileId;
use systemprompt_bridge::gateway::manifest::{
    ManifestError, SignedManifest, SignedManifestBuilder, SignedManifestEnvelope, WindowVerdict,
    check_release_window, decode_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_bridge::instance::window::{
    CandidateRefusal, CombinedWindow, admit_candidate, combine,
};
use systemprompt_models::bridge::instance::BridgeVersionWindow;
use systemprompt_test_fixtures::fixture_user_id;

fn v(s: &str) -> semver::Version {
    semver::Version::parse(s).unwrap()
}

fn window(min: Option<&str>, max: Option<&str>, enforce_max: bool) -> BridgeVersionWindow {
    BridgeVersionWindow {
        min: min.map(v),
        max: max.map(v),
        enforce_max,
    }
}

fn manifest_with(window: Option<BridgeVersionWindow>) -> SignedManifest {
    let at = chrono::DateTime::parse_from_rfc3339("2026-04-22T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let builder = SignedManifestBuilder::new(
        ManifestVersion::try_new("2026-04-22T00:00:00Z-01abcdef").unwrap(),
        at,
        at,
        fixture_user_id(),
    );
    match window {
        Some(w) => builder.with_bridge_versions(w).build(),
        None => builder.build(),
    }
}

#[test]
fn no_window_admits_every_release() {
    assert_eq!(
        check_release_window(&manifest_with(None), "0.1.0").unwrap(),
        WindowVerdict::Admitted
    );
}

#[test]
fn a_release_below_the_window_is_refused_and_names_the_minimum() {
    let err = check_release_window(
        &manifest_with(Some(window(Some("0.75.0"), None, false))),
        "0.74.3",
    )
    .unwrap_err();
    match err {
        ManifestError::BridgeBelowWindow { local, min } => {
            assert_eq!(local, "0.74.3");
            assert_eq!(min, "0.75.0");
        },
        other => panic!("expected BridgeBelowWindow, got {other:?}"),
    }
}

#[test]
fn a_release_above_the_window_is_tolerated_unless_the_maximum_is_enforced() {
    let tolerated = manifest_with(Some(window(None, Some("0.75.0"), false)));
    assert_eq!(
        check_release_window(&tolerated, "0.76.0").unwrap(),
        WindowVerdict::AboveTolerated { max: v("0.75.0") }
    );
    let enforced = manifest_with(Some(window(None, Some("0.75.0"), true)));
    assert!(matches!(
        check_release_window(&enforced, "0.76.0"),
        Err(ManifestError::BridgeAboveWindow { .. })
    ));
}

#[test]
fn the_window_compares_by_semver_precedence_so_patch_releases_sit_inside() {
    let w = manifest_with(Some(window(Some("0.74.0"), Some("0.74.9"), true)));
    assert_eq!(
        check_release_window(&w, "0.74.3").unwrap(),
        WindowVerdict::Admitted
    );
    assert_eq!(
        check_release_window(&w, "0.74.3+dynapps.2").unwrap(),
        WindowVerdict::Admitted,
        "build metadata never moves a release across a bound"
    );
}

#[test]
fn the_window_rides_the_manifest_payload_and_old_payloads_decode_without_it() {
    let manifest = manifest_with(Some(window(Some("0.75.0"), Some("0.76.0"), true)));
    let payload = serde_json::to_string(&manifest).unwrap();
    let decoded = decode_payload(&SignedManifestEnvelope {
        payload,
        signature: ManifestSignature::new(""),
    })
    .unwrap();
    assert_eq!(decoded.bridge_versions, manifest.bridge_versions);

    let mut legacy = serde_json::to_value(manifest_with(None)).unwrap();
    legacy.as_object_mut().unwrap().remove("bridge_versions");
    legacy.as_object_mut().unwrap().remove("allow_user_disable");
    let decoded = decode_payload(&SignedManifestEnvelope {
        payload: legacy.to_string(),
        signature: ManifestSignature::new(""),
    })
    .unwrap();
    assert!(decoded.bridge_versions.is_none());
    assert!(
        !decoded.allow_user_disable,
        "disable is withheld unless granted"
    );
}

fn id(s: &str) -> GatewayProfileId {
    GatewayProfileId::try_new(s).unwrap()
}

#[test]
fn an_update_must_sit_inside_every_profiles_window() {
    let combined = combine(&[
        (id("a"), window(Some("0.74.0"), Some("0.76.0"), false)),
        (id("b"), window(Some("0.75.0"), None, false)),
    ]);
    assert_eq!(
        combined,
        CombinedWindow::Window(window(Some("0.75.0"), Some("0.76.0"), false))
    );
    admit_candidate(&combined, &v("0.75.2")).unwrap();
    assert!(matches!(
        admit_candidate(&combined, &v("0.74.9")),
        Err(CandidateRefusal::Outside { .. })
    ));
}

#[test]
fn windows_that_do_not_overlap_are_a_conflict_and_admit_nothing() {
    let combined = combine(&[
        (id("a"), window(None, Some("0.74.0"), false)),
        (id("b"), window(Some("0.75.0"), None, false)),
    ]);
    assert!(matches!(combined, CombinedWindow::Conflict(_)));
    let err = admit_candidate(&combined, &v("0.74.5")).unwrap_err();
    assert!(
        matches!(err, CandidateRefusal::Conflict(ref text) if text.contains('a') && text.contains('b'))
    );
}

#[test]
fn no_delivered_window_is_open() {
    assert_eq!(combine(&[]), CombinedWindow::Open);
}
