use std::collections::HashMap;
use std::path::PathBuf;

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::client::McpClient;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::OAuthRequirement;
use systemprompt_test_fixtures::fixture_user_id;

fn external_config(external_auth: bool) -> McpServerConfig {
    McpServerConfig {
        name: "sf".to_string(),
        owner: fixture_user_id(),
        server_type: systemprompt_models::mcp::McpServerType::External,
        binary: String::new(),
        enabled: true,
        display_in_web: true,
        port: 0,
        crate_path: PathBuf::new(),
        display_name: "Salesforce".to_string(),
        description: "external".to_string(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: true,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        version: "1.0.0".to_string(),
        host: String::new(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: "https://api.salesforce.com/mcp".to_string(),
        external_auth: external_auth.then(|| systemprompt_models::mcp::ExternalAuth {
            token_endpoint: "/api/public/sf/token".to_string(),
            header: "Authorization".to_string(),
            scheme: "Bearer".to_string(),
        }),
        headers: HashMap::new(),
    }
}

fn context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::new(uuid::Uuid::new_v4().to_string()),
        AgentName::new("external-proxy-test"),
    )
}

#[tokio::test]
async fn resolve_external_proxy_target_errors_without_accessor() {
    let config = external_config(false);
    let result = McpClient::resolve_external_proxy_target(&config, &context()).await;
    assert!(
        result.is_err(),
        "an external server without external_auth cannot mint a per-user bearer",
    );
}
