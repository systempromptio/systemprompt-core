use systemprompt_identifiers::{PluginId, ServiceName};

#[test]
fn service_name_try_new_accepts_a_name() {
    let name = ServiceName::try_new("systemprompt-admin").unwrap();
    assert_eq!(name.as_str(), "systemprompt-admin");
}

#[test]
fn service_name_try_new_rejects_blank() {
    assert!(ServiceName::try_new("").is_err());
    assert!(ServiceName::try_new("   ").is_err());
}

#[test]
fn service_name_deserialize_validates() {
    let ok: ServiceName = serde_json::from_str("\"content-manager\"").unwrap();
    assert_eq!(ok, ServiceName::new("content-manager"));
    let blank: Result<ServiceName, _> = serde_json::from_str("\"\"");
    assert!(blank.is_err());
}

#[test]
fn service_name_from_str_validates() {
    assert!("agent-a".parse::<ServiceName>().is_ok());
    assert!("".parse::<ServiceName>().is_err());
}

#[test]
fn unvalidated_ids_still_offer_a_non_empty_try_new() {
    assert_eq!(PluginId::try_new("p1").unwrap().as_str(), "p1");
    assert!(PluginId::try_new("").is_err());
}
