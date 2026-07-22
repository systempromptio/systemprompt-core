//! Drives `enforce_rbac_from_registry` end-to-end through a live rmcp duplex
//! service: unknown server, anonymous access, missing bearer, invalid token,
//! full JWT authentication, and an authz-hook denial.

use std::future::Future;
use std::sync::Arc;

use rmcp::model::{ListToolsResult, PaginatedRequestParams, Tool};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::middleware::{AuthResult, enforce_rbac_from_registry};
use systemprompt_models::RequestContext as SysRequestContext;
use systemprompt_security::authz::{AllowAllHook, DenyAllHook, SharedAuthzHook};
use systemprompt_test_fixtures::{ensure_test_bootstrap, mint_admin_jwt};

use crate::harness::write_services_config;

const ISSUER: &str = "https://issuer.test";

fn sys_ctx() -> SysRequestContext {
    SysRequestContext::new(
        SessionId::new("s-rbac"),
        TraceId::new("t-rbac"),
        ContextId::generate(),
        AgentName::new("agent-rbac"),
    )
    .with_actor(Actor::user(UserId::new("user-rbac")))
}

fn server_yaml(name: &str, oauth_required: bool, scopes: &str) -> String {
    format!(
        r"mcp_servers:
  {name}:
    server_type: external
    binary: {name}-bin
    package: null
    port: 0
    endpoint: http://127.0.0.1:1/mcp
    enabled: true
    display_in_web: true
    oauth:
      required: {oauth_required}
      scopes: [{scopes}]
      audience: mcp
      client_id: null
"
    )
}

#[derive(Clone)]
struct RbacProbe {
    server: String,
    headers: Vec<(String, String)>,
    hook: SharedAuthzHook,
}

impl ServerHandler for RbacProbe {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        mut context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let probe = self.clone();
        async move {
            let mut builder = http::Request::builder().uri("http://localhost/mcp");
            for (name, value) in &probe.headers {
                builder = builder.header(name.as_str(), value.as_str());
            }
            let (mut parts, ()) = builder.body(()).expect("request").into_parts();
            parts.extensions.insert(sys_ctx());
            context.extensions.insert(parts);

            let outcome = match enforce_rbac_from_registry(&context, &probe.server, &probe.hook)
                .await
            {
                Ok(AuthResult::Anonymous(ctx)) => {
                    format!("anonymous:{}", ctx.session_id().as_str())
                },
                Ok(AuthResult::Authenticated(auth)) => format!(
                    "authenticated:user={}:token-len={}",
                    auth.context
                        .user
                        .as_ref()
                        .map(|u| u.email.clone())
                        .unwrap_or_default(),
                    auth.token().len()
                ),
                Err(err) => format!("err:{}", err.message),
            };

            Ok(ListToolsResult {
                tools: vec![Tool::new(
                    outcome,
                    "rbac outcome",
                    Arc::new(serde_json::Map::new()),
                )],
                next_cursor: None,
                meta: None,
            })
        }
    }
}

async fn probe_outcome(probe: RbacProbe) -> String {
    let (server_io, client_io) = tokio::io::duplex(8192);

    let handle = tokio::spawn(async move {
        if let Ok(running) = probe.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });

    let client = ().serve(client_io).await.expect("client handshake");
    let listed = client
        .list_tools(None)
        .await
        .expect("probe handler responds");
    let outcome = listed.tools[0].name.clone().into_owned();

    client.cancel().await.expect("client shutdown");
    handle.abort();
    outcome
}

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn unknown_server_is_rejected() {
    let bootstrap = ensure_test_bootstrap();
    write_services_config(bootstrap, &server_yaml(&unique("rbl"), false, ""));

    let outcome = probe_outcome(RbacProbe {
        server: "rbl_absent".to_owned(),
        headers: vec![],
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(outcome.contains("not found in registry"), "got: {outcome}");
}

#[tokio::test]
async fn oauth_not_required_yields_anonymous_context() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("rbl_anon");
    write_services_config(bootstrap, &server_yaml(&name, false, ""));

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![],
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert_eq!(outcome, "anonymous:s-rbac");
}

#[tokio::test]
async fn oauth_required_without_bearer_is_rejected() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("rbl_nobearer");
    write_services_config(bootstrap, &server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![],
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(outcome.contains("Authentication required"), "got: {outcome}");
}

#[tokio::test]
async fn malformed_bearer_token_is_rejected() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("rbl_badtok");
    write_services_config(bootstrap, &server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![("authorization".to_owned(), "Bearer not-a-jwt".to_owned())],
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(outcome.contains("Invalid JWT token"), "got: {outcome}");
}

#[tokio::test]
async fn valid_admin_jwt_is_authenticated() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("rbl_auth");
    write_services_config(bootstrap, &server_yaml(&name, true, "admin"));

    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user, "rbac-live@test.invalid", ISSUER);

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![(
            "authorization".to_owned(),
            format!("Bearer {}", token.as_str()),
        )],
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(
        outcome.starts_with("authenticated:user=rbac-live@test.invalid"),
        "got: {outcome}"
    );
}

#[tokio::test]
async fn insufficient_scope_is_rejected() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("rbl_scope");
    write_services_config(bootstrap, &server_yaml(&name, true, ""));

    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user, "rbac-scope@test.invalid", ISSUER);

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![(
            "authorization".to_owned(),
            format!("Bearer {}", token.as_str()),
        )],
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(
        outcome.contains("Insufficient permissions"),
        "got: {outcome}"
    );
}

#[tokio::test]
async fn deny_hook_blocks_authenticated_request() {
    let bootstrap = ensure_test_bootstrap();
    let name = unique("rbl_deny");
    write_services_config(bootstrap, &server_yaml(&name, true, "admin"));

    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    let token = mint_admin_jwt(&user, "rbac-deny@test.invalid", ISSUER);

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![(
            "authorization".to_owned(),
            format!("Bearer {}", token.as_str()),
        )],
        hook: Arc::new(DenyAllHook::null()),
    })
    .await;
    assert!(outcome.contains("authz denied"), "got: {outcome}");
}
