use systemprompt_bridge::gateway::manifest::{
    ManifestError, SignedManifestBuilder, SignedManifestEnvelope, bridge_version_is_supported,
    decode_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_test_fixtures::fixture_user_id;

fn envelope_requiring(floor: Option<&str>) -> SignedManifestEnvelope {
    let manifest = SignedManifestBuilder::new(
        ManifestVersion::try_new("2026-04-22T00:00:00Z-01abcdef").expect("version"),
        "2026-04-22T00:00:00Z",
        "2026-04-22T00:00:00Z",
        fixture_user_id(),
    )
    .build();
    let mut value = serde_json::to_value(&manifest).expect("serializes");
    value["min_bridge_version"] = floor.map_or(serde_json::Value::Null, |f| f.into());
    SignedManifestEnvelope {
        payload: value.to_string(),
        signature: ManifestSignature::new(""),
    }
}

#[test]
fn a_floor_above_this_build_is_rejected_with_both_versions_named() {
    let err = decode_payload(&envelope_requiring(Some("999.0.0")))
        .expect_err("a bridge below the floor must not sync");
    match err {
        ManifestError::BridgeTooOld { local, required } => {
            assert_eq!(required, "999.0.0");
            assert!(
                !local.is_empty(),
                "the local version is reported to the user"
            );
        },
        other => panic!("expected BridgeTooOld, got {other:?}"),
    }
}

#[test]
fn a_floor_at_or_below_this_build_is_accepted() {
    decode_payload(&envelope_requiring(Some("0.0.1"))).expect("a supported bridge syncs");
}

#[test]
fn a_gateway_that_declares_no_floor_is_accepted() {
    decode_payload(&envelope_requiring(None)).expect("an older gateway still syncs");
}

#[test]
fn the_floor_comparison_orders_numerically_not_lexically() {
    assert!(
        bridge_version_is_supported("0.1.10", "0.1.9"),
        "0.1.10 is newer than 0.1.9; a lexical compare would invert this"
    );
    assert!(!bridge_version_is_supported("0.1.9", "0.1.10"));
    assert!(
        bridge_version_is_supported("1.0.0", "1.0.0"),
        "the floor itself is supported"
    );
}

#[test]
fn an_unparseable_version_is_allowed_through() {
    assert!(
        bridge_version_is_supported("dev-build", "1.0.0"),
        "refusing dev builds would make the gateway untestable against a work tree"
    );
}
