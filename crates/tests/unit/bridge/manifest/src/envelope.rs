use systemprompt_bridge::gateway::manifest::{
    MANIFEST_INLINE_SCHEMA_VERSION, MANIFEST_SCHEMA_VERSION, ManifestError, SignedManifestBuilder,
    SignedManifestEnvelope, decode_payload,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_test_fixtures::unique_user_id;

fn version(s: &str) -> ManifestVersion {
    ManifestVersion::try_new(s).expect("valid manifest version literal")
}

fn payload_value() -> serde_json::Value {
    payload_value_for(&unique_user_id("envelope"))
}

fn payload_value_for(user_id: &UserId) -> serde_json::Value {
    let manifest = SignedManifestBuilder::new(
        version("2026-04-22T00:00:00Z-01abcdef"),
        chrono::DateTime::parse_from_rfc3339("2026-04-22T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        chrono::DateTime::parse_from_rfc3339("2026-04-22T00:00:00Z")
            .expect("rfc3339")
            .with_timezone(&chrono::Utc),
        user_id.clone(),
    )
    .with_enabled_hosts(vec![HostKind::ClaudeDesktop])
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
    let user_id = unique_user_id("envelope");
    let manifest = decode_payload(&envelope(&payload_value_for(&user_id))).expect("decodes");
    assert_eq!(manifest.user_id, user_id);
    assert_eq!(manifest.min_schema_version, MANIFEST_INLINE_SCHEMA_VERSION);
    assert_eq!(manifest.enabled_hosts, vec![HostKind::ClaudeDesktop]);
}

#[test]
fn decode_payload_ignores_unknown_fields() {
    let user_id = unique_user_id("envelope");
    let mut payload = payload_value_for(&user_id);
    payload["field_from_a_future_gateway"] = serde_json::json!({"nested": [1, 2, 3]});
    let manifest = decode_payload(&envelope(&payload)).expect("unknown fields tolerated");
    assert_eq!(manifest.user_id, user_id);
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
