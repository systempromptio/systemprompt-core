use std::collections::HashSet;
use systemprompt_identifiers::{AgentId, AgentName, DbValue, ToDbValue};

#[test]
fn agent_id_generate_produces_uuid_v4_format() {
    let id = AgentId::generate();
    assert_eq!(id.as_str().len(), 36);
    let parts: Vec<&str> = id.as_str().split('-').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 4);
    assert_eq!(parts[2].len(), 4);
    assert_eq!(parts[3].len(), 4);
    assert_eq!(parts[4].len(), 12);
}

#[test]
fn agent_id_generate_unique_across_calls() {
    let ids: HashSet<String> = (0..100)
        .map(|_| AgentId::generate().as_str().to_string())
        .collect();
    assert_eq!(ids.len(), 100);
}

#[test]
fn agent_id_display_matches_inner_value() {
    let id = AgentId::new("my-agent");
    assert_eq!(format!("{}", id), "my-agent");
}

#[test]
fn agent_id_from_string_and_str_produce_equal_values() {
    let from_str: AgentId = "test".into();
    let from_string: AgentId = String::from("test").into();
    assert_eq!(from_str, from_string);
}

#[test]
fn agent_id_into_string() {
    let id = AgentId::new("convert-me");
    let s: String = id.into();
    assert_eq!(s, "convert-me");
}

#[test]
fn agent_id_partial_eq_str() {
    let id = AgentId::new("cmp-test");
    assert!(id == "cmp-test");
    assert!("cmp-test" == id);
}

#[test]
fn agent_id_serde_transparent_json() {
    let id = AgentId::new("serde-test");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serde-test\"");
    let deserialized: AgentId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn agent_id_to_db_value_ref_and_owned() {
    let id = AgentId::new("db-test");
    let owned_val = id.to_db_value();
    let ref_val = (&id).to_db_value();
    assert!(matches!(owned_val, DbValue::String(ref s) if s == "db-test"));
    assert!(matches!(ref_val, DbValue::String(ref s) if s == "db-test"));
}

#[test]
fn agent_id_accepts_empty_string() {
    let id = AgentId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn agent_id_accepts_unicode() {
    let id = AgentId::new("agent-日本語-🤖");
    assert_eq!(id.as_str(), "agent-日本語-🤖");
}

#[test]
fn agent_name_try_new_valid() {
    let name = AgentName::try_new("valid-agent").unwrap();
    assert_eq!(name.as_str(), "valid-agent");
}

#[test]
fn agent_name_try_new_empty_returns_error() {
    let err = AgentName::try_new("").unwrap_err();
    assert_eq!(err.to_string(), "AgentName cannot be empty");
}

#[test]
fn agent_name_rejects_unknown_lowercase() {
    let err = AgentName::try_new("unknown").unwrap_err();
    assert!(err.to_string().contains("reserved"));
}

#[test]
fn agent_name_rejects_unknown_uppercase() {
    let err = AgentName::try_new("UNKNOWN").unwrap_err();
    assert!(err.to_string().contains("reserved"));
}

#[test]
fn agent_name_rejects_unknown_mixed_case() {
    let err = AgentName::try_new("UnKnOwN").unwrap_err();
    assert!(err.to_string().contains("reserved"));
}

#[test]
fn agent_name_system_returns_system_value() {
    let name = AgentName::system();
    assert_eq!(name.as_str(), "system");
}

#[test]
fn agent_name_display_matches_inner_value() {
    let name = AgentName::new("display-agent");
    assert_eq!(format!("{}", name), "display-agent");
}

#[test]
fn agent_name_serde_roundtrip_exact_json() {
    let name = AgentName::new("serde-agent");
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, "\"serde-agent\"");
    let deserialized: AgentName = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, name);
}

#[test]
fn agent_name_serde_rejects_empty_on_deserialize() {
    let result: Result<AgentName, _> = serde_json::from_str("\"\"");
    assert!(result.is_err());
}

#[test]
fn agent_name_serde_rejects_unknown_on_deserialize() {
    let result: Result<AgentName, _> = serde_json::from_str("\"unknown\"");
    assert!(result.is_err());
}

#[test]
fn agent_name_try_from_str_ref() {
    let name: AgentName = "valid".try_into().unwrap();
    assert_eq!(name.as_str(), "valid");
}

#[test]
fn agent_name_try_from_string() {
    let name: AgentName = String::from("valid").try_into().unwrap();
    assert_eq!(name.as_str(), "valid");
}

#[test]
fn agent_name_from_str_parse() {
    let name: AgentName = "valid".parse().unwrap();
    assert_eq!(name.as_str(), "valid");
}

#[test]
fn agent_name_equality_across_construction() {
    let from_new = AgentName::new("test");
    let from_try: AgentName = "test".try_into().unwrap();
    let from_parse: AgentName = "test".parse().unwrap();
    assert_eq!(from_new, from_try);
    assert_eq!(from_try, from_parse);
}

#[test]
fn agent_name_to_db_value() {
    let name = AgentName::new("db-agent");
    let db_val = name.to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "db-agent"));
}

#[test]
#[should_panic(expected = "AgentName validation failed")]
fn agent_name_new_panics_on_empty() {
    let _ = AgentName::new("");
}

#[test]
#[should_panic(expected = "'unknown' is reserved")]
fn agent_name_new_panics_on_unknown() {
    let _ = AgentName::new("unknown");
}
