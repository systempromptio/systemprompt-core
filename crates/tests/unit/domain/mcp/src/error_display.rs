use systemprompt_mcp::McpDomainError;

#[test]
fn timeout_display_contains_server_and_ms() {
    let e = McpDomainError::Timeout {
        server: "my-mcp".to_owned(),
        after_ms: 5000,
    };
    let s = e.to_string();
    assert!(s.contains("my-mcp"));
    assert!(s.contains("5000"));
}

#[test]
fn circuit_open_display_contains_server() {
    let e = McpDomainError::CircuitOpen {
        server: "circuit-srv".to_owned(),
    };
    let s = e.to_string();
    assert!(s.contains("circuit-srv"));
}

#[test]
fn dependency_unavailable_display_contains_server() {
    let e = McpDomainError::DependencyUnavailable {
        server: "dep-srv".to_owned(),
    };
    let s = e.to_string();
    assert!(s.contains("dep-srv"));
}

#[test]
fn manifest_display_contains_message() {
    let e = McpDomainError::Manifest("bad manifest".to_owned());
    let s = e.to_string();
    assert!(s.contains("bad manifest"));
}

#[test]
fn transport_display_contains_message() {
    let e = McpDomainError::Transport("broken pipe".to_owned());
    let s = e.to_string();
    assert!(s.contains("broken pipe"));
}

#[test]
fn config_validation_display_contains_message() {
    let e = McpDomainError::ConfigValidation("missing field x".to_owned());
    let s = e.to_string();
    assert!(s.contains("missing field x"));
}

#[test]
fn client_initialize_display_contains_message() {
    let e = McpDomainError::ClientInitialize("init failed".to_owned());
    let s = e.to_string();
    assert!(s.contains("init failed"));
}

#[test]
fn service_error_display_contains_message() {
    let e = McpDomainError::ServiceError {
        message: "service down".to_owned(),
    };
    let s = e.to_string();
    assert!(s.contains("service down"));
}

#[test]
fn path_display_contains_path() {
    let e = McpDomainError::Path("/no/such/path".to_owned());
    let s = e.to_string();
    assert!(s.contains("/no/such/path"));
}

#[test]
fn timeout_debug_contains_variant_name() {
    let e = McpDomainError::Timeout {
        server: "srv".to_owned(),
        after_ms: 1,
    };
    let s = format!("{e:?}");
    assert!(s.contains("Timeout"));
}

#[test]
fn circuit_open_debug_contains_variant_name() {
    let e = McpDomainError::CircuitOpen {
        server: "srv".to_owned(),
    };
    let s = format!("{e:?}");
    assert!(s.contains("CircuitOpen"));
}

#[test]
fn dependency_unavailable_debug_contains_variant_name() {
    let e = McpDomainError::DependencyUnavailable {
        server: "srv".to_owned(),
    };
    let s = format!("{e:?}");
    assert!(s.contains("DependencyUnavailable"));
}

#[test]
fn manifest_debug_contains_variant_name() {
    let e = McpDomainError::Manifest("m".to_owned());
    let s = format!("{e:?}");
    assert!(s.contains("Manifest"));
}

#[test]
fn transport_debug_contains_variant_name() {
    let e = McpDomainError::Transport("t".to_owned());
    let s = format!("{e:?}");
    assert!(s.contains("Transport"));
}

#[test]
fn error_chain_timeout_zero_ms() {
    let e = McpDomainError::Timeout {
        server: "s".to_owned(),
        after_ms: 0,
    };
    let s = e.to_string();
    assert!(s.contains("0"));
}

#[test]
fn error_chain_timeout_large_ms() {
    let e = McpDomainError::Timeout {
        server: "s".to_owned(),
        after_ms: u64::MAX,
    };
    let s = e.to_string();
    assert!(!s.is_empty());
}

#[test]
fn mcp_domain_result_ok_propagates() {
    let r: systemprompt_mcp::McpDomainResult<u32> = Ok(42);
    assert_eq!(r.unwrap(), 42);
}

#[test]
fn mcp_domain_result_err_propagates() {
    let r: systemprompt_mcp::McpDomainResult<u32> =
        Err(McpDomainError::ServerNotFound("x".to_owned()));
    assert!(r.is_err());
}
