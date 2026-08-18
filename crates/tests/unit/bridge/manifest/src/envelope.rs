use systemprompt_bridge::gateway::manifest::{
    MANIFEST_SCHEMA_VERSION, ManifestError, SignedManifestBuilder, SignedManifestEnvelope,
    decode_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_test_fixtures::fixture_user_id;

fn version(s: &str) -> ManifestVersion {
    ManifestVersion::try_new(s).expect("valid manifest version literal")
}

fn payload_value() -> serde_json::Value {
    let manifest = SignedManifestBuilder::new(
        version("2026-04-22T00:00:00Z-01abcdef"),
        "2026-04-22T00:00:00Z",
        "2026-04-22T00:00:00Z",
        fixture_user_id(),
    )
    .with_enabled_hosts(vec!["claude-desktop".into()])
    .build();
    serde_json::to_value(&manifest).expect("manifest serializes")
}

fn envelope(payload: &serde_json::Value) -> SignedManifestEnvelope {
    SignedManifestEnvelope {
        payload: payload.to_string(),
        signature: ManifestSignature::new(""),
    }
}

#[test]
fn decode_payload_round_trips_builder_manifest() {
    let manifest = decode_payload(&envelope(&payload_value())).expect("decodes");
    assert_eq!(manifest.user_id, fixture_user_id());
    assert_eq!(manifest.min_schema_version, MANIFEST_SCHEMA_VERSION);
    assert_eq!(manifest.enabled_hosts, vec!["claude-desktop"]);
}

#[test]
fn decode_payload_ignores_unknown_fields() {
    let mut payload = payload_value();
    payload["field_from_a_future_gateway"] = serde_json::json!({"nested": [1, 2, 3]});
    let manifest = decode_payload(&envelope(&payload)).expect("unknown fields tolerated");
    assert_eq!(manifest.user_id, fixture_user_id());
}

#[test]
fn decode_payload_rejects_schema_newer_than_supported() {
    let mut payload = payload_value();
    payload["min_schema_version"] = serde_json::json!(MANIFEST_SCHEMA_VERSION + 1);
    let err = decode_payload(&envelope(&payload)).expect_err("must refuse newer schema");
    match err {
        ManifestError::SchemaTooNew {
            required,
            supported,
        } => {
            assert_eq!(required, MANIFEST_SCHEMA_VERSION + 1);
            assert_eq!(supported, MANIFEST_SCHEMA_VERSION);
        },
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
}

#[test]
fn decode_payload_missing_schema_version_defaults_to_zero() {
    let mut payload = payload_value();
    payload
        .as_object_mut()
        .expect("payload is an object")
        .remove("min_schema_version");
    let manifest = decode_payload(&envelope(&payload)).expect("pre-versioned payloads decode");
    assert_eq!(manifest.min_schema_version, 0);
}

#[test]
fn decode_payload_rejects_malformed_json() {
    let env = SignedManifestEnvelope {
        payload: "{ not a manifest".to_owned(),
        signature: ManifestSignature::new(""),
    };
    let err = decode_payload(&env).expect_err("malformed payload must fail");
    assert!(matches!(err, ManifestError::PayloadParse(_)), "got {err:?}");
}
