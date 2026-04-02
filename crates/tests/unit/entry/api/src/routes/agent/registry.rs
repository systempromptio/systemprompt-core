use systemprompt_api::routes::agent::registry::create_mcp_extensions_from_config;

#[test]
fn test_create_mcp_extensions_empty_server_names_returns_empty() {
    let result = create_mcp_extensions_from_config(&[], "https://api.example.com");
    assert!(result.is_empty());
}

#[test]
fn test_create_mcp_extensions_single_server_creates_one_extension() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    assert_eq!(result.len(), 1);
}

#[test]
fn test_create_mcp_extensions_uri_is_mcp_tools() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    assert_eq!(result[0].uri, "systemprompt:mcp-tools");
}

#[test]
fn test_create_mcp_extensions_is_required() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    assert_eq!(result[0].required, Some(true));
}

#[test]
fn test_create_mcp_extensions_has_description() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    assert!(result[0].description.is_some());
    assert!(result[0].description.as_deref().unwrap().contains("MCP"));
}

#[test]
fn test_create_mcp_extensions_params_contain_servers() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    let params = result[0].params.as_ref().unwrap();
    let servers_arr = params["servers"].as_array().unwrap();
    assert_eq!(servers_arr.len(), 1);
}

#[test]
fn test_create_mcp_extensions_server_endpoint_format() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    let params = result[0].params.as_ref().unwrap();
    let server = &params["servers"][0];
    assert_eq!(server["name"], "weather");
    assert_eq!(
        server["endpoint"],
        "https://api.example.com/api/v1/mcp/weather/mcp"
    );
}

#[test]
fn test_create_mcp_extensions_multiple_servers() {
    let servers = vec![
        "weather".to_string(),
        "calendar".to_string(),
        "search".to_string(),
    ];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    assert_eq!(result.len(), 1);
    let params = result[0].params.as_ref().unwrap();
    let servers_arr = params["servers"].as_array().unwrap();
    assert_eq!(servers_arr.len(), 3);
    assert_eq!(servers_arr[0]["name"], "weather");
    assert_eq!(servers_arr[1]["name"], "calendar");
    assert_eq!(servers_arr[2]["name"], "search");
}

#[test]
fn test_create_mcp_extensions_params_contain_supported_protocols() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    let params = result[0].params.as_ref().unwrap();
    let protocols = params["supported_protocols"].as_array().unwrap();
    assert_eq!(protocols.len(), 1);
    assert_eq!(protocols[0], "2024-11-05");
}

#[test]
fn test_create_mcp_extensions_server_default_auth_status() {
    let servers = vec!["weather".to_string()];
    let result = create_mcp_extensions_from_config(&servers, "https://api.example.com");
    let params = result[0].params.as_ref().unwrap();
    let server = &params["servers"][0];
    assert_eq!(server["auth"], "unknown");
    assert_eq!(server["status"], "unknown");
}
