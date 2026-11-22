//! `handle_mcp_registry` projects the registry the `mcp` member provides into
//! the discovery document clients read. A success carries every listed server
//! under `data` with its endpoint synthesised from the server name; a
//! registry that cannot be read is the typed 5xx envelope with the fixed
//! public message, never an empty server list.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::State;
use axum::http::StatusCode;
use systemprompt_mcp_entry::http::{McpRegistryState, handle_mcp_registry};
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig};

enum Stub {
    Servers(Vec<McpServerInfo>),
    Unreadable,
}

#[async_trait]
impl McpRegistryProvider for Stub {
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError> {
        Err(RegistryError::NotFound(name.to_owned()))
    }

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError> {
        match self {
            Self::Servers(servers) => Ok(servers.clone()),
            Self::Unreadable => Err(RegistryError::Configuration(
                "services config unreadable".into(),
            )),
        }
    }

    async fn managed_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError> {
        self.list_enabled_servers().await
    }

    async fn validate(&self) -> Result<(), RegistryError> {
        self.list_enabled_servers().await.map(|_| ())
    }
}

fn server(name: &str) -> McpServerInfo {
    McpServerInfo {
        name: name.to_owned(),
        version: "2.1.0".to_owned(),
        description: "An enabled fixture MCP server".to_owned(),
        port: None,
        display_in_web: true,
        enabled: true,
        oauth: ServiceOAuthConfig {
            required: true,
            scopes: vec!["user".to_owned(), "mcp".to_owned()],
            audience: "mcp".to_owned(),
            ema: false,
        },
    }
}

async fn call(stub: Stub) -> (StatusCode, serde_json::Value) {
    let response = handle_mcp_registry(State(McpRegistryState::new(Arc::new(stub)))).await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, serde_json::from_slice(&bytes).expect("a JSON body"))
}

#[tokio::test]
async fn a_listed_server_carries_its_fields_and_a_synthesised_endpoint() {
    let (status, body) = call(Stub::Servers(vec![server("fixture_enabled_mcp")])).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let entries = body["data"].as_array().expect("servers under data");
    assert_eq!(entries.len(), 1, "{body}");
    let entry = &entries[0];
    assert_eq!(entry["name"], "fixture_enabled_mcp");
    assert_eq!(entry["version"], "2.1.0");
    assert_eq!(entry["description"], "An enabled fixture MCP server");
    assert_eq!(entry["display_in_web"], true);
    assert_eq!(entry["oauth_required"], true);
    assert_eq!(entry["oauth_scopes"], serde_json::json!(["user", "mcp"]));
    assert_eq!(entry["status"], "enabled");
    assert!(
        entry.get("port").is_none(),
        "no port is listed for an external server: {entry}"
    );
    assert_eq!(
        entry["endpoint"],
        systemprompt_models::modules::ApiPaths::mcp_server_endpoint("fixture_enabled_mcp")
    );
    assert!(body["meta"]["version"].is_string(), "{body}");
}

#[tokio::test]
async fn an_empty_registry_is_an_empty_list() {
    let (status, body) = call(Stub::Servers(Vec::new())).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], serde_json::json!([]), "{body}");
}

#[tokio::test]
async fn an_unreadable_registry_is_the_fixed_server_error_and_never_an_empty_list() {
    let (status, body) = call(Stub::Unreadable).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "internal_error", "{body}");
    assert_eq!(body["message"], "Internal server error", "{body}");
    assert!(
        !body.to_string().contains("services config unreadable"),
        "the cause stays in the log: {body}"
    );
    assert!(!body["data"].is_array(), "{body}");
}
