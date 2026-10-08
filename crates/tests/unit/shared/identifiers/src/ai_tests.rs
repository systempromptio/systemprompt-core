use std::collections::HashSet;
use systemprompt_identifiers::{
    AiGatewayPolicyId, AiQuotaBucketId, AiRequestId, AiSafetyFindingId, ConfigId, DbValue,
    MessageId, ToDbValue,
};

#[test]
fn ai_request_id_generate_uuid_format() {
    let id = AiRequestId::generate();
    assert_eq!(id.as_str().len(), 36);
    assert_eq!(id.as_str().chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn ai_request_id_generate_unique() {
    let ids: HashSet<String> = (0..20)
        .map(|_| AiRequestId::generate().as_str().to_string())
        .collect();
    assert_eq!(ids.len(), 20);
}

#[test]
fn ai_request_id_serde_transparent_json() {
    let id = AiRequestId::new("req-123");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"req-123\"");
    let deserialized: AiRequestId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn ai_request_id_from_string_and_str_equal() {
    let from_str: AiRequestId = "test".into();
    let from_string: AiRequestId = String::from("test").into();
    assert_eq!(from_str, from_string);
}

#[test]
fn ai_request_id_into_string() {
    let id = AiRequestId::new("convert");
    let s: String = id.into();
    assert_eq!(s, "convert");
}

#[test]
fn ai_request_id_to_db_value() {
    let id = AiRequestId::new("db");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "db"));
}

#[test]
fn message_id_generate_uuid_format() {
    let id = MessageId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn message_id_generate_unique() {
    let id1 = MessageId::generate();
    let id2 = MessageId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn message_id_serde_transparent_json() {
    let id = MessageId::new("msg-123");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"msg-123\"");
}

#[test]
fn config_id_generate_uuid_format() {
    let id = ConfigId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn config_id_generate_unique() {
    let id1 = ConfigId::generate();
    let id2 = ConfigId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn config_id_serde_transparent_json() {
    let id = ConfigId::generate();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, format!("\"{id}\""));
    let back: ConfigId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn config_id_rejects_non_uuid() {
    assert!(ConfigId::try_new("cfg-123").is_err());
    assert!(serde_json::from_str::<ConfigId>("\"cfg-123\"").is_err());
}

#[test]
fn uuid_arm_ai_ids_round_trip_through_uuid() {
    let finding = AiSafetyFindingId::generate();
    let bucket = AiQuotaBucketId::generate();
    let policy = AiGatewayPolicyId::generate();
    assert_eq!(
        AiSafetyFindingId::from_uuid(finding.to_uuid().unwrap()),
        finding
    );
    assert_eq!(
        AiQuotaBucketId::from_uuid(bucket.to_uuid().unwrap()),
        bucket
    );
    assert_eq!(
        AiGatewayPolicyId::from_uuid(policy.to_uuid().unwrap()),
        policy
    );
    assert!(AiGatewayPolicyId::try_new("policy-1").is_err());
}
