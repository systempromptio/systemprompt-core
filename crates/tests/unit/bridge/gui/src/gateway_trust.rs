use systemprompt_bridge::gui_policy::trust::{TrustConfirmError, confirm, key_fingerprint};
use systemprompt_identifiers::ValidatedUrl;

const KEY: &str = "MCowBQYDK2VwAyEAq0c8k2y1QO0yWkVb2b2xYwTzP2r6l1m0n9o8p7q6r5s=";
const OTHER_KEY: &str = "MCowBQYDK2VwAyEA0000000000000000000000000000000000000000000=";

fn gateway() -> ValidatedUrl {
    ValidatedUrl::try_new("https://gateway.example.test").expect("url")
}

#[test]
fn the_fingerprint_is_the_sha256_of_the_served_key() {
    let fp = key_fingerprint(KEY);
    assert_eq!(fp.len(), 64);
    assert!(
        fp.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(key_fingerprint(&format!("{KEY}\n")), fp);
    assert_ne!(key_fingerprint(OTHER_KEY), fp);
}

#[test]
fn a_confirmed_fingerprint_for_the_active_gateway_returns_the_key_to_pin() {
    let gw = gateway();
    let fp = key_fingerprint(KEY);
    assert_eq!(confirm(gw.as_str(), &fp, &gw, KEY), Ok(KEY));
    let spaced = fp
        .as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).expect("hex").to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(confirm(gw.as_str(), &spaced, &gw, KEY), Ok(KEY));
}

#[test]
fn a_key_rotated_after_review_is_not_pinned() {
    let gw = gateway();
    let reviewed = key_fingerprint(KEY);
    assert!(matches!(
        confirm(gw.as_str(), &reviewed, &gw, OTHER_KEY),
        Err(TrustConfirmError::FingerprintMismatch { .. })
    ));
}

#[test]
fn a_gateway_switched_after_review_is_not_pinned() {
    let active = ValidatedUrl::try_new("https://other.example.test").expect("url");
    let reviewed = key_fingerprint(KEY);
    assert!(matches!(
        confirm(gateway().as_str(), &reviewed, &active, KEY),
        Err(TrustConfirmError::GatewayChanged { .. })
    ));
}

#[test]
fn an_empty_or_truncated_fingerprint_is_refused() {
    let gw = gateway();
    let fp = key_fingerprint(KEY);
    for confirmed in ["", &fp[..16]] {
        assert!(
            confirm(gw.as_str(), confirmed, &gw, KEY).is_err(),
            "{confirmed}"
        );
    }
}
