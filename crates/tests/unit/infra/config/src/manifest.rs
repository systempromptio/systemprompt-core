use base64::Engine;
use systemprompt_config::bootstrap::{
    MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed,
};

#[test]
fn generate_seed_returns_random_32_bytes() {
    let a = generate_seed();
    let b = generate_seed();
    assert_eq!(a.len(), MANIFEST_SIGNING_SEED_BYTES);
    assert_ne!(a, b, "two random seeds should differ");
}

#[test]
fn decode_seed_round_trips_base64() {
    let seed = generate_seed();
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);
    let decoded = decode_seed(&encoded).unwrap();
    assert_eq!(decoded, seed);
}

#[test]
fn decode_seed_trims_surrounding_whitespace() {
    let seed = generate_seed();
    let encoded = base64::engine::general_purpose::STANDARD.encode(seed);
    let padded = format!("  {encoded}\n");
    let decoded = decode_seed(&padded).unwrap();
    assert_eq!(decoded, seed);
}

#[test]
fn decode_seed_rejects_wrong_length() {
    let short = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
    let err = decode_seed(&short).unwrap_err();
    assert!(format!("{err}").contains("byte seed"));
}

#[test]
fn decode_seed_rejects_invalid_base64() {
    let err = decode_seed("not-base64!!!").unwrap_err();
    assert!(format!("{err}").contains("base64"));
}

#[test]
fn persist_seed_inserts_field_into_existing_secrets() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, br#"{"oauth_at_rest_pepper": "abc"}"#).unwrap();
    let seed = generate_seed();
    persist_seed(&path, &seed).unwrap();

    let body = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let stored = v["manifest_signing_secret_seed"].as_str().unwrap();
    let decoded = decode_seed(stored).unwrap();
    assert_eq!(decoded, seed);
    assert_eq!(v["oauth_at_rest_pepper"].as_str(), Some("abc"));
}

#[test]
fn persist_seed_overwrites_existing_seed_field() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(
        &path,
        br#"{"oauth_at_rest_pepper":"abc","manifest_signing_secret_seed":"OLD"}"#,
    )
    .unwrap();
    let seed = generate_seed();
    persist_seed(&path, &seed).unwrap();

    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let stored = v["manifest_signing_secret_seed"].as_str().unwrap();
    assert_ne!(stored, "OLD");
    assert_eq!(decode_seed(stored).unwrap(), seed);
}

#[test]
fn persist_seed_errors_when_file_root_is_not_object() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, b"[]").unwrap();
    let err = persist_seed(&path, &generate_seed()).unwrap_err();
    assert!(format!("{err}").contains("not a JSON object"));
}

#[test]
fn persist_seed_errors_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("missing.json");
    let err = persist_seed(&path, &generate_seed()).unwrap_err();
    assert!(!format!("{err}").is_empty());
}
