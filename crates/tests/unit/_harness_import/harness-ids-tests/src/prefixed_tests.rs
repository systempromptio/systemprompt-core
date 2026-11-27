use std::collections::HashSet;
use std::str::FromStr;

use harness_ids::{IdError, JobId, MessageId, PartId, ProjectId, SessionId, now_ms};

#[test]
fn minted_ids_are_unique_and_sort_in_creation_order() {
    let ids: Vec<MessageId> = (0..5000).map(|_| MessageId::mint()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(sorted, ids);
    let unique: HashSet<&str> = ids.iter().map(MessageId::as_str).collect();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn string_order_matches_creation_order() {
    let first = SessionId::mint();
    let second = SessionId::mint();
    assert!(first.as_str() < second.as_str());
    assert!(first < second);
}

#[test]
fn each_kind_carries_its_prefix() {
    assert!(ProjectId::mint().as_str().starts_with("prj_"));
    assert!(SessionId::mint().as_str().starts_with("ses_"));
    assert!(MessageId::mint().as_str().starts_with("msg_"));
    assert!(PartId::mint().as_str().starts_with("prt_"));
    assert!(JobId::mint().as_str().starts_with("job_"));
    assert_eq!(JobId::PREFIX, "job");
    assert_eq!(ProjectId::PREFIX, "prj");
}

#[test]
fn minted_id_has_fixed_shape() {
    let id = PartId::mint();
    let body = id.as_str().strip_prefix("prt_").unwrap();
    assert_eq!(body.len(), 26);
    assert!(
        body.bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    );
}

#[test]
fn parse_round_trips_a_minted_id() {
    let id = MessageId::mint();
    assert_eq!(MessageId::parse(id.as_str()).unwrap(), id);
    assert_eq!(MessageId::from_str(id.as_str()).unwrap(), id);
    assert_eq!(MessageId::try_from(String::from(id.clone())).unwrap(), id);
}

#[test]
fn parse_rejects_another_kind_and_malformed_bodies() {
    let message = MessageId::mint();
    let error = SessionId::parse(message.as_str()).unwrap_err();
    assert_eq!(
        error,
        IdError {
            kind: "session",
            value: message.as_str().to_owned()
        }
    );
    assert!(MessageId::parse("msg_short").is_err());
    assert!(MessageId::parse("msg").is_err());
    assert!(MessageId::parse("").is_err());
    assert!(MessageId::parse(&message.as_str().to_uppercase().replacen("MSG", "msg", 1)).is_err());
    assert!(MessageId::parse(&message.as_str().replacen('_', "-", 1)).is_err());
}

#[test]
fn error_names_the_kind_and_the_value() {
    let error = JobId::parse("nope").unwrap_err();
    assert_eq!(error.to_string(), "\"nope\" is not a valid job id");
}

#[test]
fn created_at_reads_back_the_mint_time() {
    let before = now_ms();
    let id = ProjectId::mint();
    let after = now_ms();
    assert!(id.created_at_ms() >= before - 1);
    assert!(id.created_at_ms() <= after + 1);
}

#[test]
fn display_and_as_ref_are_the_raw_string() {
    let id = SessionId::mint();
    assert_eq!(id.to_string(), id.as_str());
    let as_ref: &str = id.as_ref();
    assert_eq!(as_ref, id.as_str());
}

#[test]
fn serde_uses_the_plain_string_and_validates_on_read() {
    let id = PartId::mint();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, format!("\"{}\"", id.as_str()));
    let back: PartId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
    assert!(serde_json::from_str::<PartId>("\"prt_bad\"").is_err());
    assert!(serde_json::from_str::<SessionId>(&json).is_err());
}
