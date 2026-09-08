use systemprompt_bridge::sync::SyncError;
use systemprompt_bridge::{config, install};
use systemprompt_identifiers::ValidatedUrl;

const KEY: &str = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";

#[test]
fn missing_pin_error_offers_explicit_tofu() {
    let message = SyncError::PubkeyNotPinned.to_string();
    assert!(message.contains("not pinned"));
    assert!(message.contains("--allow-tofu"));
}

#[test]
fn bridge_policy_binds_validated_key_to_gateway() {
    let gateway = ValidatedUrl::try_new("https://gateway.example").expect("gateway");
    let values = install::bridge_policy_values(Some(KEY), &gateway).expect("policy");
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].0, "manifestTrust");
    assert_eq!(values[0].1, "REG_SZ");
    let record: serde_json::Value = serde_json::from_str(&values[0].2).expect("trust JSON");
    assert_eq!(record["gateway"], "https://gateway.example");
    assert_eq!(record["key"], KEY);
    assert_eq!(record["source"], "policy");
    assert!(
        install::bridge_policy_values(None, &gateway)
            .expect("absent")
            .is_empty()
    );
    assert!(install::bridge_policy_values(Some("not a key"), &gateway).is_err());
    assert_ne!(
        config::store::bridge_policy_subkey(),
        r"SOFTWARE\Policies\Claude"
    );
}

#[test]
fn unbound_policy_pubkey_is_not_policy_trust() {
    temp_env::with_vars(
        [
            ("SP_BRIDGE_POLICY_PUBKEY", Some(KEY)),
            ("SP_BRIDGE_POLICY_TRUST", None),
        ],
        || {
            assert_eq!(
                config::policy_pubkey().expect("a bare manifestPubkey is not an error"),
                None,
                "a manifestPubkey without a gateway binding never counts as managed trust"
            );
        },
    );
}

#[test]
fn bound_policy_returns_key() {
    let record =
        serde_json::json!({"gateway":"https://gateway.example", "key":KEY,"source":"policy"})
            .to_string();
    temp_env::with_var("SP_BRIDGE_POLICY_TRUST", Some(record), || {
        assert_eq!(
            config::policy_pubkey()
                .expect("policy")
                .expect("key")
                .as_str(),
            KEY
        );
    });
}

#[test]
fn malformed_policy_is_an_error() {
    temp_env::with_var("SP_BRIDGE_POLICY_TRUST", Some("invalid json"), || {
        assert!(matches!(
            config::policy_pubkey(),
            Err(config::TrustError::InvalidPolicy(_))
        ));
    });
}

#[test]
fn is_uuid_like_requires_standard_hyphenation() {
    assert!(install::is_uuid_like(
        "f8e4d915-f8ad-5304-ab0d-c1bf895df963"
    ));
    for invalid in [
        "",
        "not-a-uuid",
        "{f8e4d915-f8ad-5304-ab0d-c1bf895df963}",
        "f8e4d915f8ad5304ab0dc1bf895df963",
    ] {
        assert!(!install::is_uuid_like(invalid));
    }
}
