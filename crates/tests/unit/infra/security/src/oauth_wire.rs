//! Token-endpoint `expires_in` spellings (RFC 6749 §5.1) and the JWS `crit`
//! header rule (RFC 7515 §4.1.11).

use jsonwebtoken::{Algorithm, Header};
use serde::Deserialize;
use systemprompt_security::jwt::reject_critical_header;

#[derive(Deserialize)]
struct Response {
    #[serde(
        default,
        deserialize_with = "systemprompt_security::oauth_wire::expires_in"
    )]
    expires_in: Option<u64>,
}

fn parse(body: &str) -> Option<u64> {
    serde_json::from_str::<Response>(body)
        .expect("token response parses")
        .expires_in
}

#[test]
fn expires_in_accepts_number_string_null_and_absence() {
    assert_eq!(parse(r#"{"expires_in": 3599}"#), Some(3599));
    assert_eq!(parse(r#"{"expires_in": "3599"}"#), Some(3599));
    assert_eq!(parse(r#"{"expires_in": null}"#), None);
    assert_eq!(parse("{}"), None);
}

#[test]
fn an_unreadable_expires_in_is_absent_not_fatal() {
    assert_eq!(parse(r#"{"expires_in": "soon"}"#), None);
    assert_eq!(parse(r#"{"expires_in": -5}"#), None);
    assert_eq!(parse(r#"{"expires_in": {"s": 1}}"#), None);
}

#[test]
fn a_crit_header_is_refused_and_its_absence_accepted() {
    let mut header = Header::new(Algorithm::RS256);
    assert!(reject_critical_header(&header).is_ok());
    header.crit = Some(vec!["b64".to_owned()]);
    assert!(reject_critical_header(&header).is_err());
    header.crit = Some(Vec::new());
    assert!(reject_critical_header(&header).is_err());
}
