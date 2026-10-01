use systemprompt_bridge::gui::state::{VerifiedIdentity, decode_jwt_identity_unverified};

// Hand-built unsigned JWT. The payload segment is the URL_SAFE_NO_PAD base64
// of {"email":"a@b.com","sub":"00000000-0000-4000-8000-000000000001",
// "tenant_id":"tenant_1","exp":1893456000}.
const VALID_TOKEN: &str = "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6ImFAYi5jb20iLCJzdWIiOiIwMDAwMDAwMC0wMDAwLTQwMDAtODAwMC0wMDAwMDAwMDAwMDEiLCJ0ZW5hbnRfaWQiOiJ0ZW5hbnRfMSIsImV4cCI6MTg5MzQ1NjAwMH0.sig";

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
    assert_eq!(
        user_id.as_ref().map(|id| id.as_str()),
        Some("00000000-0000-4000-8000-000000000001")
    );
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

// Payload segments are base64url of `{"sub":""}` and `{"sub":"unset"}`.
const EMPTY_SUB_TOKEN: &str = "eyJhbGciOiJub25lIn0.eyJzdWIiOiIifQ.sig";
const SENTINEL_SUB_TOKEN: &str = "eyJhbGciOiJub25lIn0.eyJzdWIiOiJ1bnNldCJ9.sig";

// Payload segment is base64url of `{"email":"a@b.com","sub":"test-user"}`.
const OPAQUE_SUB_TOKEN: &str =
    "eyJhbGciOiJub25lIn0.eyJlbWFpbCI6ImFAYi5jb20iLCJzdWIiOiJ0ZXN0LXVzZXIifQ.sig";

#[test]
fn empty_or_sentinel_subject_decodes_to_no_identity() {
    assert!(decode_jwt_identity_unverified(EMPTY_SUB_TOKEN).is_none());
    assert!(decode_jwt_identity_unverified(SENTINEL_SUB_TOKEN).is_none());
}

#[test]
fn opaque_non_uuid_subject_still_decodes() {
    let identity =
        decode_jwt_identity_unverified(OPAQUE_SUB_TOKEN).expect("opaque subject decodes");
    assert_eq!(
        identity.user_id.as_ref().map(|id| id.as_str()),
        Some("test-user")
    );
    assert_eq!(identity.email.as_deref(), Some("a@b.com"));
}
