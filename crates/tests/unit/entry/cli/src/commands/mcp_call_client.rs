//! Tests for the `plugins mcp call` client helpers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use rmcp::model::ContentBlock;
use systemprompt_cli::plugins::mcp::call_client::{
    ToolCallParams, convert_content, execute_tool_call, list_available_tools,
};
use systemprompt_cli::session::CliSessionContext;
use systemprompt_cloud::{CliSession, SessionIdentity};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

fn session_ctx() -> CliSessionContext {
    let session = CliSession::builder(
        ProfileName::new("test"),
        SessionToken::new("tok"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new("user-mcp-call"),
            Email::new("a@b.test"),
            UserType::Admin,
        ),
    )
    .build();
    CliSessionContext {
        session,
        profile: minimal_profile(),
    }
}

fn minimal_profile() -> Profile {
    Profile {
        name: "test".to_string(),
        display_name: "Test".to_string(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Test Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_string(),
            api_internal_url: "http://localhost:8080".to_string(),
            api_external_url: "https://example.com".to_string(),
            use_https: false,
            cors_allowed_origins: vec![],
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            trusted_proxies: Vec::new(),
        },
        paths: PathsConfig {
            system: "/tmp/test".to_string(),
            services: "/tmp/test/services".to_string(),
            bin: "/tmp/test/bin".to_string(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "https://issuer.test".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![systemprompt_models::auth::JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        providers: systemprompt_models::profile::ProviderRegistry::default(),
        gateway: None,
        governance: None,
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
        },
    }
}

fn block(json: serde_json::Value) -> ContentBlock {
    serde_json::from_value(json).unwrap()
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[test]
fn convert_content_maps_text_image_audio_and_resource_link() {
    let text = convert_content(&block(serde_json::json!({"type": "text", "text": "hello"})));
    assert_eq!(text.kind, "text");
    assert_eq!(text.text.as_deref(), Some("hello"));

    let image = convert_content(&block(serde_json::json!({
        "type": "image", "data": "aGk=", "mimeType": "image/png"
    })));
    assert_eq!(image.kind, "image");
    assert_eq!(image.mime_type.as_deref(), Some("image/png"));
    assert_eq!(image.data.as_deref(), Some("aGk="));

    let audio = convert_content(&block(serde_json::json!({
        "type": "audio", "data": "aGk=", "mimeType": "audio/wav"
    })));
    assert_eq!(audio.kind, "audio");
    assert_eq!(audio.mime_type.as_deref(), Some("audio/wav"));

    let link = convert_content(&block(serde_json::json!({
        "type": "resource_link", "uri": "file:///x", "name": "x", "mimeType": "text/plain"
    })));
    assert_eq!(link.kind, "resource_link");
    assert_eq!(link.text.as_deref(), Some("file:///x"));
    assert_eq!(link.mime_type.as_deref(), Some("text/plain"));
}

#[test]
fn convert_content_wraps_embedded_resources_as_debug_text() {
    let resource = convert_content(&block(serde_json::json!({
        "type": "resource",
        "resource": {"uri": "file:///doc", "text": "body", "mimeType": "text/plain"}
    })));
    assert_eq!(resource.kind, "resource");
    assert!(resource.text.unwrap().contains("file:///doc"));
}

#[tokio::test]
async fn execute_tool_call_fails_fast_against_closed_port() {
    let ctx = session_ctx();
    let err = execute_tool_call(ToolCallParams {
        server_name: "svc",
        port: free_port(),
        tool_name: "echo",
        arguments: Some(serde_json::json!({"x": 1})),
        session_ctx: &ctx,
        timeout_secs: 5,
    })
    .await
    .unwrap_err();

    assert!(err.to_string().contains("Failed to connect to MCP server"));
}

#[tokio::test]
async fn list_available_tools_fails_fast_against_closed_port() {
    let ctx = session_ctx();
    let err = list_available_tools("svc", free_port(), &ctx, 5)
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Failed to connect to MCP server"));
}
