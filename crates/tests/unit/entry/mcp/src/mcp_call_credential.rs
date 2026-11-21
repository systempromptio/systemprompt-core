//! The platform session reaches only a server the platform hosts: an
//! internal server is called with the caller's own session, an external
//! server never receives it, and an external server that requires a
//! credential is refused by name.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use std::path::PathBuf;

use systemprompt_extension::cli::CallerSession;
use systemprompt_identifiers::{ContextId, SessionId, SessionToken, UserId};
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp_entry::cli::call_client::{
    CredentialRefused, McpTarget, ServerCredential, list_tools, server_credential,
};
use systemprompt_models::auth::{JwtAudience, UserType};
use systemprompt_models::mcp::{McpServerType, OAuthRequirement};

const TOKEN: &str = "platform-session-token";

const CALLER_CONTEXT: &str = "7c1d5e9a-3b2f-4c8d-9e6a-1f0b2c3d4e5f";

pub(crate) fn server_config(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: UserId::new("owner"),
        server_type: McpServerType::Internal,
        binary: Some("server".to_owned()),
        enabled: true,
        display_in_web: true,
        port: Some(5100),
        crate_path: PathBuf::from("/srv/mcp"),
        display_name: name.to_owned(),
        description: String::new(),
        capabilities: vec!["tools".to_owned()],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
            service_only: false,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        secrets: Vec::new(),
        version: "1.0.0".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: HashMap::new(),
    }
}

fn external(name: &str, endpoint: &str) -> McpServerConfig {
    let mut config = server_config(name);
    config.server_type = McpServerType::External;
    config.binary = None;
    config.port = None;
    config.remote_endpoint = endpoint.to_owned();
    config
}

fn caller() -> CallerSession {
    CallerSession::new(
        SessionToken::new(TOKEN),
        SessionId::new("sess-caller"),
        UserId::new("user-caller"),
        ContextId::try_new(CALLER_CONTEXT).expect("a v4 context id"),
        UserType::Admin,
    )
}

#[test]
fn an_internal_server_is_presented_the_caller_session() {
    let mut oauth = server_config("internal-oauth");
    oauth.oauth.required = true;
    for server in [server_config("internal"), oauth] {
        assert_eq!(
            server_credential(&server).unwrap(),
            ServerCredential::CallerSession,
            "{}",
            server.name
        );
    }
}

#[test]
fn an_external_server_without_a_credential_requirement_is_presented_nothing() {
    let server = external("open", "https://mcp.example.com/mcp");
    assert_eq!(server_credential(&server).unwrap(), ServerCredential::Withheld);

    let caller = caller();
    let target = McpTarget::resolve(&server, Some(&caller)).unwrap();
    assert!(
        !target.presents_caller_session(),
        "a session offered for an external server is withheld"
    );
    assert_eq!(target.url(), "https://mcp.example.com/mcp");
}

#[test]
fn an_external_server_requiring_a_credential_is_refused_by_name() {
    let mut oauth = external("ext-oauth", "https://oauth.example.com/mcp");
    oauth.oauth.required = true;
    let mut brokered = external("ext-brokered", "https://brokered.example.com/mcp");
    brokered.external_auth = Some(
        serde_json::from_value(serde_json::json!({"token_endpoint": "/api/v1/broker/token"}))
            .unwrap(),
    );

    for server in [oauth, brokered] {
        let refused = server_credential(&server).unwrap_err();
        assert!(
            matches!(&refused, CredentialRefused::ExternalServer { server: id } if id.as_str() == server.name),
            "{refused}"
        );
        assert!(refused.to_string().contains(&server.name), "{refused}");

        let caller = caller();
        let error = McpTarget::resolve(&server, Some(&caller)).unwrap_err();
        assert!(
            error.downcast_ref::<CredentialRefused>().is_some(),
            "{error:#}"
        );
    }
}

#[test]
fn an_internal_server_without_the_caller_session_is_refused() {
    let error = McpTarget::resolve(&server_config("internal"), None).unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<CredentialRefused>(),
            Some(CredentialRefused::MissingCallerSession { .. })
        ),
        "{error:#}"
    );
}

#[tokio::test]
async fn an_external_endpoint_never_receives_the_session_or_the_callers_ids() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let endpoint = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&endpoint)
        .await;

    let server = external("open", &format!("{}/mcp", endpoint.uri()));
    let caller = caller();
    let target = McpTarget::resolve(&server, Some(&caller)).unwrap();
    let _refused_by_fixture = list_tools(&target, 5).await;

    let requests = endpoint
        .received_requests()
        .await
        .expect("recorded MCP requests");
    assert!(!requests.is_empty(), "the client reached the endpoint");
    for request in &requests {
        assert!(
            request.headers.get("authorization").is_none(),
            "no credential is presented to an external endpoint"
        );
        for (name, value) in &request.headers {
            let value = value.to_str().unwrap_or_default();
            for secret in [TOKEN, "sess-caller", "user-caller", CALLER_CONTEXT] {
                assert!(
                    !value.contains(secret),
                    "header {name} carries the caller's {secret}"
                );
            }
        }
    }
}

#[test]
fn the_caller_session_attributes_a_request_to_the_caller() {
    let context = caller().request_context(systemprompt_identifiers::AgentName::system());
    assert_eq!(context.user_id().as_str(), "user-caller");
    assert_eq!(context.context_id().as_str(), CALLER_CONTEXT);
    assert_eq!(context.session_id().as_str(), "sess-caller");
    assert_eq!(
        context.auth_token().map(|token| token.as_str()),
        Some(TOKEN)
    );
}

#[tokio::test]
async fn a_platform_hosted_server_is_called_as_the_caller() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let endpoint = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&endpoint)
        .await;

    let mut server = server_config("hosted");
    server.port = Some(endpoint.address().port());
    let caller = caller();
    let target = McpTarget::resolve(&server, Some(&caller)).unwrap();
    assert!(target.presents_caller_session());
    let _refused_by_fixture = list_tools(&target, 5).await;

    let requests = endpoint
        .received_requests()
        .await
        .expect("recorded MCP requests");
    let first = requests.first().expect("the client reached the server");
    let headers: Vec<String> = first
        .headers
        .iter()
        .map(|(_, value)| value.to_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        headers.iter().any(|value| value == &format!("Bearer {TOKEN}")),
        "the caller's token is presented: {headers:?}"
    );
    for id in ["sess-caller", CALLER_CONTEXT] {
        assert!(
            headers.iter().any(|value| value == id),
            "the request carries the caller's own {id}: {headers:?}"
        );
    }
}
