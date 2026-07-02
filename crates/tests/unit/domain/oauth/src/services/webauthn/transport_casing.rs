use serde_json::json;
use systemprompt_oauth::services::webauthn::service::normalize_transport_casing;

fn passkey_json() -> serde_json::Value {
    json!({ "cred": { "transports": [] } })
}

fn transports_after(stored: &[&str]) -> Vec<String> {
    let mut value = passkey_json();
    let stored: Vec<String> = stored.iter().map(|s| (*s).to_owned()).collect();
    normalize_transport_casing(&mut value, &stored);
    value["cred"]["transports"]
        .as_array()
        .expect("transports array")
        .iter()
        .map(|v| v.as_str().expect("transport string").to_owned())
        .collect()
}

#[test]
fn recases_known_transports_to_webauthn_rs_variant_casing() {
    assert_eq!(
        transports_after(&["internal", "usb", "nfc", "ble", "hybrid"]),
        vec!["Internal", "Usb", "Nfc", "Ble", "Hybrid"]
    );
}

#[test]
fn recases_stored_values_case_insensitively() {
    assert_eq!(
        transports_after(&["INTERNAL", "Usb", "NfC"]),
        vec!["Internal", "Usb", "Nfc"]
    );
}

#[test]
fn passes_unknown_transports_through_lowercased() {
    assert_eq!(
        transports_after(&["test", "SmartCard"]),
        vec!["test", "smartcard"]
    );
}

#[test]
fn leaves_value_untouched_when_cred_key_is_missing() {
    let mut value = json!({ "other": true });
    normalize_transport_casing(&mut value, &["internal".to_owned()]);
    assert_eq!(value, json!({ "other": true }));
}

#[test]
fn replaces_existing_transports_with_stored_set() {
    let mut value = json!({ "cred": { "transports": ["Usb"] } });
    normalize_transport_casing(&mut value, &["internal".to_owned()]);
    assert_eq!(value["cred"]["transports"], json!(["Internal"]));
}
