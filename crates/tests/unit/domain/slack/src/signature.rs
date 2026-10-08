use systemprompt_slack::signature::{MAX_TIMESTAMP_SKEW_SECS, sign, verify_slack_signature};

const SECRET: &[u8] = b"8f742231b10e8888abcd99yyyzzz85a5";
const TS: &str = "1531420618";
const BODY: &[u8] = b"token=xyz&team_id=T1DC2JH3J&command=%2Fweather";

#[test]
fn round_trip_valid_signature_passes() {
    let sig = sign(SECRET, TS, BODY).expect("sign");
    assert!(verify_slack_signature(SECRET, TS, &sig, BODY, 1_531_420_618).is_ok());
}

#[test]
fn wrong_secret_fails() {
    let sig = sign(SECRET, TS, BODY).expect("sign");
    assert!(verify_slack_signature(b"other-secret", TS, &sig, BODY, 1_531_420_618).is_err());
}

#[test]
fn tampered_body_fails() {
    let sig = sign(SECRET, TS, BODY).expect("sign");
    assert!(
        verify_slack_signature(SECRET, TS, &sig, b"token=xyz&tampered", 1_531_420_618).is_err()
    );
}

#[test]
fn stale_timestamp_rejected() {
    let sig = sign(SECRET, TS, BODY).expect("sign");
    let now = 1_531_420_618 + MAX_TIMESTAMP_SKEW_SECS + 1;
    assert!(verify_slack_signature(SECRET, TS, &sig, BODY, now).is_err());
}

#[test]
fn timestamp_within_skew_accepted() {
    let sig = sign(SECRET, TS, BODY).expect("sign");
    let now = 1_531_420_618 + MAX_TIMESTAMP_SKEW_SECS - 1;
    assert!(verify_slack_signature(SECRET, TS, &sig, BODY, now).is_ok());
}

#[test]
fn missing_v0_prefix_fails() {
    let sig = sign(SECRET, TS, BODY).expect("sign");
    let no_prefix = sig.trim_start_matches("v0=");
    assert!(verify_slack_signature(SECRET, TS, no_prefix, BODY, 1_531_420_618).is_err());
}

#[test]
fn non_numeric_timestamp_fails() {
    let sig = sign(SECRET, "not-a-number", BODY).expect("sign");
    assert!(verify_slack_signature(SECRET, "not-a-number", &sig, BODY, 1_531_420_618).is_err());
}

#[test]
fn empty_signing_secret_is_refused_even_with_a_matching_signature() {
    let sig = sign(b"", TS, BODY).expect("sign");
    assert!(
        verify_slack_signature(b"", TS, &sig, BODY, 1_531_420_618).is_err(),
        "an HMAC keyed with an empty secret is forgeable and must never verify"
    );
}
