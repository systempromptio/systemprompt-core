//! Tests for the `plugins mcp call` client helpers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use rmcp::model::ContentBlock;
use systemprompt_cli::plugins::mcp::call_client::{
    ToolCallParams, convert_content, execute_tool_call, list_available_tools,
};
use systemprompt_cli::session::CliSessionContext;
use systemprompt_cloud::{CliSession, SessionBinding, SessionIdentity};
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
        SessionBinding::new(
            ProfileName::try_new("test").expect("valid ProfileName"),
            "http://localhost:8080".to_owned(),
        ),
        SessionToken::new("tok"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new("user-mcp-call"),
            Email::try_new("a@b.test").expect("valid Email"),
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
        storage: Default::default(),
        observability: Default::default(),
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
            metrics_port: None,
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
            allow_dynamic_client_registration: true,
            login_page_url: None,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        governance: None,
        judge: Default::default(),
        services: Default::default(),
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
            email: None,
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
        url: &format!("http://127.0.0.1:{}/mcp", free_port()),
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
    let err = list_available_tools(
        "svc",
        &format!("http://127.0.0.1:{}/mcp", free_port()),
        &ctx,
        5,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("Failed to connect to MCP server"));
}

struct JsonRpcMethod(&'static str);

impl wiremock::Match for JsonRpcMethod {
    fn matches(&self, request: &wiremock::Request) -> bool {
        serde_json::from_slice::<serde_json::Value>(&request.body)
            .ok()
            .and_then(|body| {
                body.get("method")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .as_deref()
            == Some(self.0)
    }
}

struct ExactAuthorization(&'static str);

impl wiremock::Match for ExactAuthorization {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let values = request.headers.get_all("authorization");
        let values = values.iter().collect::<Vec<_>>();
        !values.is_empty()
            && values
                .iter()
                .all(|value| value.to_str().ok() == Some(self.0))
    }
}

async fn mount_protocol(server: &wiremock::MockServer) {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, ResponseTemplate};

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "systemprompt-cli-strict",
                    "version": "1.0.0"
                }
            }
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-cli-live")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "strict-fixture", "version": "1.0.0"}
                    }
                })),
        )
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

async fn assert_initialize_frame(server: &wiremock::MockServer) {
    let requests = server
        .received_requests()
        .await
        .expect("recorded MCP requests");
    let initialize_request = requests
        .iter()
        .find(|request| wiremock::Match::matches(&JsonRpcMethod("initialize"), request))
        .expect("recorded initialize request");
    let authorization = initialize_request
        .headers
        .get_all("authorization")
        .iter()
        .map(|value| value.to_str().expect("ASCII authorization value"))
        .collect::<Vec<_>>();
    assert!(
        !authorization.is_empty(),
        "authorization must be propagated"
    );
    assert!(
        authorization.iter().all(|value| *value == "Bearer tok"),
        "every propagated credential must equal the session token: {authorization:?}"
    );
    let initialize: serde_json::Value =
        serde_json::from_slice(&initialize_request.body).expect("initialize JSON-RPC body");
    assert_eq!(
        initialize,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "systemprompt-cli-strict",
                    "version": "1.0.0"
                }
            }
        })
    );
}

#[tokio::test]
async fn list_available_tools_sends_non_null_object_params_and_preserves_names() {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    mount_protocol(&server).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {"_meta": {"progressToken": 0}}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"tools": [
                        {"name": "alpha", "description": "A", "inputSchema": {"type": "object"}},
                        {"name": "zeta", "description": "Z", "inputSchema": {"type": "object"}}
                    ]}
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let tools_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        list_available_tools(
            "strict",
            &format!("{}/mcp", server.uri()),
            &session_ctx(),
            5,
        ),
    )
    .await
    .expect("tools/list must finish within ten seconds");
    assert_initialize_frame(&server).await;
    let tools = tools_result.expect("strict tools/list succeeds");
    assert_eq!(tools, vec!["alpha", "zeta"]);
    let requests = server
        .received_requests()
        .await
        .expect("recorded MCP requests");
    let list_request = requests
        .iter()
        .map(|request| serde_json::from_slice::<serde_json::Value>(&request.body))
        .find_map(|body| {
            let body = body.ok()?;
            (body["method"] == "tools/list").then_some(body)
        })
        .expect("recorded tools/list request");
    assert_eq!(
        list_request["params"],
        serde_json::json!({"_meta": {"progressToken": 0}})
    );
    assert!(
        list_request["params"].get("cursor").is_none(),
        "the initial list request must not invent a pagination cursor: {list_request}"
    );
}

#[tokio::test]
async fn execute_tool_call_sends_arguments_and_surfaces_jsonrpc_rejection() {
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let success = MockServer::start().await;
    mount_protocol(&success).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call",
            "params": {"name": "echo", "arguments": {"message": "owned-value"}}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "content": [{"type": "text", "text": "owned-result"}],
                        "isError": false
                    }
                })),
        )
        .expect(1)
        .mount(&success)
        .await;
    let success_ctx = session_ctx();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        execute_tool_call(ToolCallParams {
            server_name: "strict",
            url: &format!("{}/mcp", success.uri()),
            tool_name: "echo",
            arguments: Some(serde_json::json!({"message": "owned-value"})),
            session_ctx: &success_ctx,
            timeout_secs: 5,
        }),
    )
    .await
    .expect("tools/call must finish within ten seconds");
    assert_initialize_frame(&success).await;
    let result = result.expect("strict tools/call succeeds");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
    assert_eq!(
        convert_content(&result.content[0]).text.as_deref(),
        Some("owned-result")
    );

    let rejected = MockServer::start().await;
    mount_protocol(&rejected).await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(ExactAuthorization("Bearer tok"))
        .and(header("mcp-session-id", "sess-cli-live"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call",
            "params": {"name": "denied"}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": {"code": -32602, "message": "owned rejection"}
                })),
        )
        .expect(1)
        .mount(&rejected)
        .await;
    let ctx = session_ctx();
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        execute_tool_call(ToolCallParams {
            server_name: "strict",
            url: &format!("{}/mcp", rejected.uri()),
            tool_name: "denied",
            arguments: None,
            session_ctx: &ctx,
            timeout_secs: 5,
        }),
    )
    .await
    .expect("rejected tools/call must finish within ten seconds");
    assert_initialize_frame(&rejected).await;
    let error = error.expect_err("JSON-RPC rejection must fail the call");
    let chain = format!("{error:#}");
    assert!(
        chain.contains("MCP tool 'denied' on 'strict' rejected the call"),
        "{chain}"
    );
    assert!(chain.contains("owned rejection"), "{chain}");
}
