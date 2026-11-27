use chrono::{DateTime, Utc};
use sp_protocol::{DeviceCredential, HookToken, Jwt, JwtError, OneTimeCode, Pat, SecretError};

const TOKEN: &str = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1MSIsImV4cCI6MTg5MzQ1NjAwMCwic2Vzc2lvbl9pZCI6InNlc3MtMSJ9.c2ln";

#[test]
fn debug_and_display_redact() {
    let pat = Pat::new("sp-live-abcdef").unwrap();
    assert_eq!(format!("{pat:?}"), "Pat(<redacted>)");
    assert_eq!(format!("{pat}"), "<redacted>");
    assert_eq!(pat.expose(), "sp-live-abcdef");
    let jwt = Jwt::new(TOKEN).unwrap();
    assert!(!format!("{jwt:?}").contains("eyJ"));
}

#[test]
fn pasted_values_are_trimmed_and_empty_refused() {
    assert_eq!(Pat::new("  sp-live-x \n").unwrap().expose(), "sp-live-x");
    assert_eq!(OneTimeCode::new(" abc ").unwrap().expose(), "abc");
    assert!(matches!(Pat::new("   "), Err(SecretError::Empty { .. })));
    assert!(Jwt::new("").is_err());
}

#[test]
fn jwt_claims_are_read_without_verifying() {
    let jwt = Jwt::new(TOKEN).unwrap();
    assert_eq!(jwt.expires_at().unwrap().timestamp(), 1_893_456_000);
    assert_eq!(jwt.session_id().as_deref(), Some("sess-1"));
    let before = DateTime::<Utc>::from_timestamp(1_893_455_000, 0).unwrap();
    assert!(!jwt.is_expired(before, chrono::Duration::seconds(30)));
    assert!(jwt.is_expired(before, chrono::Duration::seconds(1_000)));
}

#[test]
fn a_token_without_exp_counts_as_expired() {
    let jwt = Jwt::new("not-a-jwt").unwrap();
    assert!(jwt.expires_at().is_none());
    assert!(jwt.is_expired(Utc::now(), chrono::Duration::zero()));
    assert!(matches!(jwt.unverified_claims(), Err(JwtError::Shape)));
}

#[test]
fn device_credentials_are_shaped() {
    assert!(DeviceCredential::new("sp_device_AbC.9~+/=-").is_ok());
    assert!(DeviceCredential::new("sp_device_").is_err());
    assert!(DeviceCredential::new("sp-live-abc").is_err());
    assert!(DeviceCredential::new("sp_device_a b").is_err());
    assert!(DeviceCredential::new(format!("sp_device_{}", "a".repeat(250))).is_err());
}

#[test]
fn secrets_deserialize_from_strings() {
    let token: HookToken = serde_json::from_str("\"tok\"").unwrap();
    assert_eq!(token.expose(), "tok");
    assert!(serde_json::from_str::<DeviceCredential>("\"nope\"").is_err());
}
