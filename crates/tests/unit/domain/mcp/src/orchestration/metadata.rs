use systemprompt_mcp::orchestration::{McpServerConnectionInfo, McpServerMetadata};

fn make_metadata(name: &str, endpoint: &str, status: &str) -> McpServerMetadata {
    McpServerMetadata {
        name: name.to_owned(),
        endpoint: endpoint.to_owned(),
        auth: "anon".to_owned(),
        status: status.to_owned(),
        version: None,
        tools: None,
    }
}

#[test]
fn metadata_fields_round_trip_serde() {
    let m = make_metadata("demo", "https://example.com/mcp", "running");
    let json = serde_json::to_string(&m).expect("serialize");
    let decoded: McpServerMetadata = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.name, "demo");
    assert_eq!(decoded.endpoint, "https://example.com/mcp");
    assert_eq!(decoded.status, "running");
    assert_eq!(decoded.auth, "anon");
}

#[test]
fn metadata_version_none_not_in_json() {
    let m = make_metadata("srv", "http://localhost/mcp", "idle");
    let json = serde_json::to_string(&m).expect("serialize");
    assert!(!json.contains("version"));
    assert!(!json.contains("tools"));
}

#[test]
fn metadata_version_some_in_json() {
    let mut m = make_metadata("srv", "http://localhost/mcp", "running");
    m.version = Some("0.9.0".to_owned());
    let json = serde_json::to_string(&m).expect("serialize");
    assert!(json.contains("0.9.0"));
}

#[test]
fn metadata_clone_eq() {
    let a = make_metadata("srv", "http://localhost/mcp", "running");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn metadata_debug_contains_name() {
    let m = make_metadata("debug-srv", "http://localhost/mcp", "ok");
    let s = format!("{m:?}");
    assert!(s.contains("debug-srv"));
}

#[test]
fn metadata_not_started_status() {
    let m = make_metadata("x", "http://x/mcp", "not_started");
    assert_eq!(m.status, "not_started");
}

#[test]
fn metadata_not_in_config_status() {
    let m = make_metadata("x", "http://x/mcp", "not_in_config");
    assert_eq!(m.status, "not_in_config");
}

#[test]
fn metadata_endpoint_synthesis_format() {
    let base = "https://api.example.com";
    let server = "my-server";
    let endpoint = format!("{base}/api/v1/mcp/{server}/mcp");
    let m = make_metadata(server, &endpoint, "running");
    assert!(m.endpoint.ends_with("/my-server/mcp"));
    assert!(m.endpoint.starts_with("https://api.example.com"));
}

#[test]
fn metadata_auth_anon() {
    let m = make_metadata("srv", "http://x/mcp", "ok");
    assert_eq!(m.auth, "anon");
}

#[test]
fn metadata_auth_user() {
    let mut m = make_metadata("srv", "http://x/mcp", "ok");
    m.auth = "user".to_owned();
    assert_eq!(m.auth, "user");
}

#[test]
fn metadata_equality_differs_on_status() {
    let a = make_metadata("srv", "http://x/mcp", "running");
    let b = make_metadata("srv", "http://x/mcp", "stopped");
    assert_ne!(a, b);
}

#[test]
fn metadata_tools_some_serialized() {
    let mut m = make_metadata("srv", "http://x/mcp", "running");
    m.tools = Some(vec![]);
    let json = serde_json::to_string(&m).expect("serialize");
    assert!(json.contains("tools"));
}

#[test]
fn connection_info_full() {
    let info = McpServerConnectionInfo {
        name: "conn-srv".to_owned(),
        display_name: Some("Connection Server".to_owned()),
        description: Some("A server for connections".to_owned()),
        host: "192.168.0.1".to_owned(),
        port: 4321,
    };
    assert_eq!(info.name, "conn-srv");
    assert_eq!(info.port, 4321);
    assert_eq!(info.display_name.as_deref(), Some("Connection Server"));
}

#[test]
fn connection_info_port_boundary_max() {
    let info = McpServerConnectionInfo {
        name: "srv".to_owned(),
        display_name: None,
        description: None,
        host: "localhost".to_owned(),
        port: u16::MAX,
    };
    assert_eq!(info.port, 65535);
}

#[test]
fn connection_info_port_boundary_min() {
    let info = McpServerConnectionInfo {
        name: "srv".to_owned(),
        display_name: None,
        description: None,
        host: "localhost".to_owned(),
        port: 0,
    };
    assert_eq!(info.port, 0);
}
