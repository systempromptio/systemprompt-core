use systemprompt_bridge::gui::state::{VerifiedIdentity, decode_jwt_identity_unverified};

// Hand-built unsigned JWT. The payload segment is the URL_SAFE_NO_PAD base64
// of {"email":"a@b.com","sub":"user_1","tenant_id":"tenant_1","exp":
// 1893456000}.
const VALID_TOKEN: &str = "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6ImFAYi5jb20iLCJzdWIiOiJ1c2VyXzEiLCJ0ZW5hbnRfaWQiOiJ0ZW5hbnRfMSIsImV4cCI6MTg5MzQ1NjAwMH0.sig";

// Payload segment is base64url of `{}` (all claims absent).
const EMPTY_CLAIMS_TOKEN: &str = "eyJhbGciOiJub25lIn0.e30.sig";

// Payload segment is base64url of the bytes `not json at all`.
const NON_JSON_TOKEN: &str = "eyJhbGciOiJub25lIn0.bm90IGpzb24gYXQgYWxs.sig";

#[test]
fn decodes_full_claims() {
    let identity = decode_jwt_identity_unverified(VALID_TOKEN).expect("token should decode");
    let VerifiedIdentity {
        email,
        user_id,
        tenant_id,
        exp_unix,
        verified_at_unix: _,
    } = identity;

    assert_eq!(email.as_deref(), Some("a@b.com"));
    assert_eq!(user_id.as_ref().map(|id| id.as_str()), Some("user_1"));
    assert_eq!(tenant_id.as_ref().map(|id| id.as_str()), Some("tenant_1"));
    assert_eq!(exp_unix, Some(1_893_456_000));
}

#[test]
fn missing_optional_fields_decode_to_none() {
    let identity =
        decode_jwt_identity_unverified(EMPTY_CLAIMS_TOKEN).expect("empty claims should decode");

    assert!(identity.email.is_none());
    assert!(identity.user_id.is_none());
    assert!(identity.tenant_id.is_none());
    assert!(identity.exp_unix.is_none());
}

#[test]
fn fewer_than_two_parts_is_none() {
    assert!(decode_jwt_identity_unverified("header-only").is_none());
}

#[test]
fn non_base64_payload_is_none() {
    assert!(decode_jwt_identity_unverified("header.*not*base64*.sig").is_none());
}

#[test]
fn non_json_payload_is_none() {
    assert!(decode_jwt_identity_unverified(NON_JSON_TOKEN).is_none());
}
