use systemprompt_agent::models::a2a::ServiceStatusParams;

#[test]
fn service_status_params_serde_roundtrip_full() {
    let params = ServiceStatusParams {
        status: "running".to_string(),
        default: true,
        port: Some(8080),
        pid: Some(12345),
    };
    let json = serde_json::to_string(&params).unwrap();
    let de: ServiceStatusParams = serde_json::from_str(&json).unwrap();
    assert_eq!(de.status, "running");
    assert!(de.default);
    assert_eq!(de.port, Some(8080));
    assert_eq!(de.pid, Some(12345));
}

#[test]
fn service_status_params_serde_roundtrip_minimal() {
    let params = ServiceStatusParams {
        status: "stopped".to_string(),
        default: false,
        port: None,
        pid: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    let de: ServiceStatusParams = serde_json::from_str(&json).unwrap();
    assert_eq!(de.status, "stopped");
    assert!(!de.default);
    assert!(de.port.is_none());
    assert!(de.pid.is_none());
}

#[test]
fn service_status_params_json_skips_none_fields() {
    let params = ServiceStatusParams {
        status: "idle".to_string(),
        default: false,
        port: None,
        pid: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(!json.contains("\"port\""));
    assert!(!json.contains("\"pid\""));
}

#[test]
fn service_status_params_json_includes_port_and_pid_when_set() {
    let params = ServiceStatusParams {
        status: "running".to_string(),
        default: false,
        port: Some(9000),
        pid: Some(99),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"port\""));
    assert!(json.contains("\"pid\""));
    assert!(json.contains("9000"));
    assert!(json.contains("99"));
}

#[test]
fn service_status_params_default_field_defaults_to_false() {
    let json = r#"{"status":"starting"}"#;
    let de: ServiceStatusParams = serde_json::from_str(json).unwrap();
    assert_eq!(de.status, "starting");
    assert!(!de.default);
    assert!(de.port.is_none());
    assert!(de.pid.is_none());
}

#[test]
fn service_status_params_debug() {
    let params = ServiceStatusParams {
        status: "running".to_string(),
        default: false,
        port: Some(8080),
        pid: None,
    };
    let dbg = format!("{:?}", params);
    assert!(dbg.contains("ServiceStatusParams"));
    assert!(dbg.contains("running"));
}

#[test]
fn service_status_params_clone_eq() {
    let params = ServiceStatusParams {
        status: "running".to_string(),
        default: true,
        port: Some(3000),
        pid: Some(1),
    };
    let cloned = params.clone();
    assert_eq!(params, cloned);
}

#[test]
fn service_status_params_camel_case_keys() {
    let json = r#"{"status":"running","default":false,"port":8080,"pid":100}"#;
    let de: ServiceStatusParams = serde_json::from_str(json).unwrap();
    assert_eq!(de.port, Some(8080));
    assert_eq!(de.pid, Some(100));
}
