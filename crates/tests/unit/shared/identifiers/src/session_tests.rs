use systemprompt_identifiers::{SessionId, SessionSource, DbValue, ToDbValue};

#[test]
fn session_id_generate_has_sess_prefix() {
    let id = SessionId::generate();
    assert!(id.as_str().starts_with("sess_"));
}

#[test]
fn session_id_generate_contains_uuid_after_prefix() {
    let id = SessionId::generate();
    let uuid_part = &id.as_str()[5..];
    assert_eq!(uuid_part.len(), 36);
    assert_eq!(uuid_part.chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn session_id_generate_unique() {
    let id1 = SessionId::generate();
    let id2 = SessionId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn session_id_system_value() {
    let id = SessionId::system();
    assert_eq!(id.as_str(), "system");
}

#[test]
fn session_id_display_format() {
    let id = SessionId::new("sess_abc123");
    assert_eq!(format!("{}", id), "sess_abc123");
}

#[test]
fn session_id_serde_transparent_json() {
    let id = SessionId::new("sess_test");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"sess_test\"");
    let deserialized: SessionId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn session_id_to_db_value() {
    let id = SessionId::new("sess_db");
    let db_val = id.to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "sess_db"));
}

#[test]
fn session_source_as_str_all_variants() {
    assert_eq!(SessionSource::Web.as_str(), "web");
    assert_eq!(SessionSource::Api.as_str(), "api");
    assert_eq!(SessionSource::Cli.as_str(), "cli");
    assert_eq!(SessionSource::Oauth.as_str(), "oauth");
    assert_eq!(SessionSource::Mcp.as_str(), "mcp");
    assert_eq!(SessionSource::Unknown.as_str(), "unknown");
}

#[test]
fn session_source_display_matches_as_str() {
    assert_eq!(format!("{}", SessionSource::Web), "web");
    assert_eq!(format!("{}", SessionSource::Api), "api");
    assert_eq!(format!("{}", SessionSource::Cli), "cli");
    assert_eq!(format!("{}", SessionSource::Oauth), "oauth");
    assert_eq!(format!("{}", SessionSource::Mcp), "mcp");
    assert_eq!(format!("{}", SessionSource::Unknown), "unknown");
}

#[test]
fn session_source_from_str_all_variants() {
    assert_eq!("web".parse::<SessionSource>().unwrap(), SessionSource::Web);
    assert_eq!("api".parse::<SessionSource>().unwrap(), SessionSource::Api);
    assert_eq!("cli".parse::<SessionSource>().unwrap(), SessionSource::Cli);
    assert_eq!("oauth".parse::<SessionSource>().unwrap(), SessionSource::Oauth);
    assert_eq!("mcp".parse::<SessionSource>().unwrap(), SessionSource::Mcp);
    assert_eq!("unknown".parse::<SessionSource>().unwrap(), SessionSource::Unknown);
}

#[test]
fn session_source_from_str_case_insensitive() {
    assert_eq!("WEB".parse::<SessionSource>().unwrap(), SessionSource::Web);
    assert_eq!("Api".parse::<SessionSource>().unwrap(), SessionSource::Api);
    assert_eq!("CLI".parse::<SessionSource>().unwrap(), SessionSource::Cli);
}

#[test]
fn session_source_from_str_unknown_for_unrecognized() {
    assert_eq!("gibberish".parse::<SessionSource>().unwrap(), SessionSource::Unknown);
    assert_eq!("".parse::<SessionSource>().unwrap(), SessionSource::Unknown);
}

#[test]
fn session_source_from_client_id_web() {
    assert_eq!(SessionSource::from_client_id("sp_web"), SessionSource::Web);
}

#[test]
fn session_source_from_client_id_cli() {
    assert_eq!(SessionSource::from_client_id("sp_cli"), SessionSource::Cli);
}

#[test]
fn session_source_from_client_id_unknown_for_others() {
    assert_eq!(SessionSource::from_client_id("sp_mobile_ios"), SessionSource::Unknown);
    assert_eq!(SessionSource::from_client_id("client_abc"), SessionSource::Unknown);
    assert_eq!(SessionSource::from_client_id(""), SessionSource::Unknown);
}

#[test]
fn session_source_default_is_unknown() {
    assert_eq!(SessionSource::default(), SessionSource::Unknown);
}

#[test]
fn session_source_serde_lowercase_format() {
    let json = serde_json::to_string(&SessionSource::Web).unwrap();
    assert_eq!(json, "\"web\"");
    let json = serde_json::to_string(&SessionSource::Oauth).unwrap();
    assert_eq!(json, "\"oauth\"");
}

#[test]
fn session_source_serde_roundtrip() {
    let source = SessionSource::Mcp;
    let json = serde_json::to_string(&source).unwrap();
    let deserialized: SessionSource = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, source);
}
