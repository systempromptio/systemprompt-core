//! Drives `enforce_rbac_from_registry` end-to-end through a live rmcp duplex
//! service, with each request admitted by the hop layer exactly as the router
//! admits it: unknown server, anonymous access, a missing or forged identity,
//! hop-token authentication, scope checks, an authz-hook denial, and the
//! hop identity's roles reaching the hook.

use std::future::Future;
use std::sync::Arc;
use systemprompt_identifiers::McpServerId;

use rmcp::model::{ListToolsResult, PaginatedRequestParams, Tool};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use systemprompt_governance::authz::{
    AllowAllHook, AuthzDecision, AuthzDecisionHook, AuthzRequest, DenyAllHook, DenyReason,
    SharedAuthzHook,
};
use systemprompt_identifiers::UserId;
use systemprompt_mcp::middleware::{
    AuthResult, HopVerifier, admit_request, enforce_rbac_from_registry,
};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_test_fixtures::{FIXTURE_EXTERNAL_URL, mint_admin_jwt, mint_mcp_hop};

use crate::harness::bootstrap_with_services;
use systemprompt_test_fixtures::unique_name;

const ISSUER: &str = FIXTURE_EXTERNAL_URL;

fn server_yaml(name: &str, oauth_required: bool, scopes: &str) -> String {
    format!(
        r"mcp_servers:
  {name}:
    server_type: external
    endpoint: http://127.0.0.1:1/mcp
    enabled: true
    tool_policy: allow
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
    admit: bool,
}

impl ServerHandler for RbacProbe {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        mut context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let probe = self.clone();
        async move {
            let mut builder = http::Request::builder()
                .uri("http://localhost/mcp")
                .header("x-session-id", "s-rbac")
                .header("x-trace-id", "t-rbac")
                .header("x-agent-name", "agent-rbac");
            for (name, value) in &probe.headers {
                builder = builder.header(name.as_str(), value.as_str());
            }
            let (mut parts, ()) = builder.body(()).expect("request").into_parts();
            let server_id = McpServerId::new(probe.server.as_str());
            let verifier = HopVerifier::new(
                systemprompt_test_fixtures::test_signing_keys()
                    .keyring()
                    .clone(),
                ISSUER,
                server_id.clone(),
            );
            if probe.admit {
                admit_request(&verifier, &parts.headers, &mut parts.extensions).expect("admitted");
            }
            context.extensions.insert(parts);

            let services = crate::harness::services_state().config();
            let outcome = match enforce_rbac_from_registry(
                &context,
                &server_id,
                &probe.hook,
                &services,
            )
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
                    auth.auth_token().map_or(0, |t| t.as_str().len())
                ),
                Err(err) => format!("err:{}", err.message),
            };

            Ok(ListToolsResult::with_all_items(vec![Tool::new(
                outcome,
                "rbac outcome",
                Arc::new(serde_json::Map::new()),
            )]))
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

fn relay() -> String {
    let token = mint_admin_jwt(
        &UserId::new(uuid::Uuid::new_v4().to_string()),
        "relay@test.invalid",
        ISSUER,
    );
    format!("Bearer {}", token.as_str())
}

fn hop_headers(
    server: &str,
    email: &str,
    permissions: Vec<Permission>,
    roles: &[&str],
) -> Vec<(String, String)> {
    let user = AuthenticatedUser::new_with_roles(
        UserId::new(uuid::Uuid::new_v4().to_string()),
        email.to_owned(),
        email.to_owned(),
        permissions,
        roles.iter().map(|role| (*role).to_owned()).collect(),
    );
    vec![
        (
            "x-systemprompt-mcp-hop".to_owned(),
            mint_mcp_hop(&McpServerId::new(server), user, None, ISSUER),
        ),
        ("authorization".to_owned(), relay()),
    ]
}

fn forged_headers() -> Vec<(String, String)> {
    vec![
        ("x-proxy-verified".to_owned(), "true".to_owned()),
        ("x-user-id".to_owned(), uuid::Uuid::new_v4().to_string()),
        ("x-user-permissions".to_owned(), "admin".to_owned()),
        ("x-user-roles".to_owned(), "analyst".to_owned()),
        ("authorization".to_owned(), relay()),
    ]
}

#[tokio::test]
async fn unknown_server_is_rejected() {
    let _bootstrap = bootstrap_with_services(&server_yaml(&unique_name("rbl"), false, ""));

    let outcome = probe_outcome(RbacProbe {
        server: "rbl_absent".to_owned(),
        headers: vec![],
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(outcome.contains("not found in registry"), "got: {outcome}");
}

#[tokio::test]
async fn oauth_not_required_yields_anonymous_context() {
    let name = unique_name("rbl_anon");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, false, ""));

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![],
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert_eq!(outcome, "anonymous:s-rbac");
}

#[tokio::test]
async fn oauth_required_without_a_hop_token_is_rejected() {
    let name = unique_name("rbl_nohop");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: vec![],
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(
        outcome.contains("Authentication required"),
        "got: {outcome}"
    );
}

#[tokio::test]
async fn forged_proxy_identity_headers_are_not_an_identity() {
    let name = unique_name("rbl_forged");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        server: name,
        headers: forged_headers(),
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(
        outcome.contains("Authentication required"),
        "got: {outcome}"
    );
}

#[tokio::test]
async fn a_valid_hop_token_is_authenticated_with_the_relayed_bearer() {
    let name = unique_name("rbl_auth");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        headers: hop_headers(
            &name,
            "rbac-live@test.invalid",
            vec![Permission::Admin],
            &[],
        ),
        server: name,
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(
        outcome.starts_with("authenticated:user=rbac-live@test.invalid"),
        "got: {outcome}"
    );
    assert!(!outcome.ends_with("token-len=0"), "got: {outcome}");
}

#[tokio::test]
async fn insufficient_scope_is_rejected() {
    let name = unique_name("rbl_scope");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        headers: hop_headers(
            &name,
            "rbac-scope@test.invalid",
            vec![Permission::User],
            &[],
        ),
        server: name,
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(
        outcome.contains("Insufficient permissions"),
        "got: {outcome}"
    );
}

#[tokio::test]
async fn oauth_required_with_no_declared_scopes_is_rejected() {
    let name = unique_name("rbl_noscope");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, ""));

    let outcome = probe_outcome(RbacProbe {
        headers: hop_headers(
            &name,
            "rbac-noscope@test.invalid",
            vec![Permission::Admin],
            &[],
        ),
        server: name,
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(outcome.contains("declares no scopes"), "got: {outcome}");
}

#[tokio::test]
async fn deny_hook_blocks_an_authenticated_request() {
    let name = unique_name("rbl_deny");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        headers: hop_headers(
            &name,
            "rbac-deny@test.invalid",
            vec![Permission::Admin],
            &[],
        ),
        server: name,
        admit: true,
        hook: Arc::new(DenyAllHook::null()),
    })
    .await;
    assert!(outcome.contains("authz denied"), "got: {outcome}");
}

#[derive(Debug)]
struct RequireRoleHook(&'static str);

#[async_trait::async_trait]
impl AuthzDecisionHook for RequireRoleHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        if req.roles.iter().any(|role| role == self.0) {
            return AuthzDecision::Allow;
        }
        let policy = "rule-based".to_owned();
        AuthzDecision::Deny {
            reason: DenyReason::NotAssigned {
                entity: req.entity,
                user_id: req.user_id,
                roles: req.roles,
            },
            policy,
        }
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        self.evaluate(req).await
    }
}

#[tokio::test]
async fn hop_identity_roles_reach_the_authz_hook() {
    let name = unique_name("rbl_roles");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "user"));

    let allowed = probe_outcome(RbacProbe {
        headers: hop_headers(
            &name,
            "r@test.invalid",
            vec![Permission::User],
            &["viewer", "analyst"],
        ),
        server: name.clone(),
        admit: true,
        hook: Arc::new(RequireRoleHook("analyst")),
    })
    .await;
    assert!(allowed.starts_with("authenticated:"), "got: {allowed}");

    let other_role = probe_outcome(RbacProbe {
        headers: hop_headers(&name, "r@test.invalid", vec![Permission::User], &["viewer"]),
        server: name.clone(),
        admit: true,
        hook: Arc::new(RequireRoleHook("analyst")),
    })
    .await;
    assert!(other_role.contains("authz denied"), "got: {other_role}");

    let forged_role = probe_outcome(RbacProbe {
        headers: forged_headers(),
        server: name,
        admit: true,
        hook: Arc::new(RequireRoleHook("analyst")),
    })
    .await;
    assert!(
        forged_role.contains("Authentication required"),
        "got: {forged_role}"
    );
}

#[tokio::test]
async fn a_verified_hop_without_a_relay_bearer_is_authenticated() {
    let name = unique_name("rbl_norelay");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let headers = hop_headers(
        &name,
        "rbac-norelay@test.invalid",
        vec![Permission::Admin],
        &[],
    )
    .into_iter()
    .filter(|(header, _)| header != "authorization")
    .collect();
    let outcome = probe_outcome(RbacProbe {
        headers,
        server: name,
        admit: true,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert_eq!(
        outcome, "authenticated:user=rbac-norelay@test.invalid:token-len=0",
        "got: {outcome}"
    );
}

#[tokio::test]
async fn a_hop_token_on_a_router_without_the_verifier_fails_loud() {
    let name = unique_name("rbl_unverified");
    let _bootstrap = bootstrap_with_services(&server_yaml(&name, true, "admin"));

    let outcome = probe_outcome(RbacProbe {
        headers: hop_headers(
            &name,
            "rbac-unverified@test.invalid",
            vec![Permission::Admin],
            &[],
        ),
        server: name,
        admit: false,
        hook: Arc::new(AllowAllHook::null()),
    })
    .await;
    assert!(outcome.contains("create_router"), "got: {outcome}");
}
