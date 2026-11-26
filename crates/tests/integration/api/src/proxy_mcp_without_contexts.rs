//! The MCP proxy in the `flavour-mcp` composition: `mcp` admitted, no
//! `agent` or `agent_runs` member, so nothing provides a context directory.
//!
//! A contextless CLI session initializes and lists tools through the mounted
//! API router, and any context a request names is replaced by the context
//! derived from the verified session. Where a member provides a directory the
//! ownership check is as strict as with the agent member: a context the caller
//! does not own, a lookup that fails, a malformed context id and a caller no
//! credential verifies are refused before the backend sees anything, on a
//! managed and on an external server. A directory withheld on this host is
//! refused the same way: it cannot be read, so it is not taken for absent.

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use http::StatusCode;
use systemprompt::reference;
use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_extension::cli::CallerSession;
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, ProvideError, Resolved,
};
use systemprompt_identifiers::{
    ContextId, ExtensionId, McpServerId, ServiceName, SessionToken, UserId,
};
use systemprompt_mcp_entry::cli::probe::{McpTarget, list_tools};
use systemprompt_models::auth::UserType;
use systemprompt_runtime::AppContext;
use systemprompt_security::jwt::{AdminTokenParams, JwtService, decode_mcp_hop};
use systemprompt_test_fixtures::{
    AppContextSeeds as _, AuthedFixture, fixture_admitted_of, fixture_core_members,
    seed_user_row_with_roles, seed_user_session,
};
use systemprompt_traits::{
    CallerContexts, ContextDirectory, ContextProviderError, DynContextDirectory,
    OwnedContextSummary,
};
use uuid::Uuid;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const COMPOSITION_MEMBERS: &str = include_str!("../../../../../scripts/composition-members.txt");

fn flavour_mcp_members() -> BTreeSet<&'static str> {
    COMPOSITION_MEMBERS
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next() == Some("flavour-mcp")).then(|| fields.next())
        })
        .flatten()
        .expect("scripts/composition-members.txt names flavour-mcp")
        .split(',')
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Directory {
    Absent,
    OwnsOnly,
    Fails,
    Withheld,
}

struct StubDirectory {
    owner: UserId,
    owned: ContextId,
    fails: bool,
}

impl ContextDirectory for StubDirectory {
    fn for_caller(&self, caller: &UserId) -> Box<dyn CallerContexts + '_> {
        Box::new(StubCaller {
            directory: self,
            caller: caller.clone(),
        })
    }
}

struct StubCaller<'a> {
    directory: &'a StubDirectory,
    caller: UserId,
}

#[async_trait]
impl CallerContexts for StubCaller<'_> {
    async fn owned_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Option<OwnedContextSummary>, ContextProviderError> {
        if self.directory.fails {
            return Err(ContextProviderError::Internal(Box::new(
                std::io::Error::other("context store unreachable"),
            )));
        }
        Ok(
            (self.caller == self.directory.owner && *context_id == self.directory.owned).then(
                || OwnedContextSummary {
                    context_id: context_id.clone(),
                    name: "owned".to_owned(),
                },
            ),
        )
    }

    async fn owned_names(
        &self,
        _context_ids: &[ContextId],
    ) -> Result<HashMap<ContextId, String>, ContextProviderError> {
        Ok(HashMap::new())
    }
}

struct DirectoryMember {
    owner: UserId,
    owned: ContextId,
    mode: Directory,
}

impl Extension for DirectoryMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "probe-context-directory",
            name: "Context directory probe",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        let directory: DynContextDirectory = Arc::new(StubDirectory {
            owner: self.owner.clone(),
            owned: self.owned.clone(),
            fails: self.mode == Directory::Fails,
        });
        let withheld = self.mode == Directory::Withheld;
        c.provide(
            move |_: &Resolved<'_>| -> Result<DynContextDirectory, ProvideError> {
                if withheld {
                    return Err(ProvideError::Withheld);
                }
                Ok(Arc::clone(&directory))
            },
        );
        Ok(())
    }
}

struct Harness {
    api: String,
    backend: MockServer,
    open_backend: MockServer,
    ctx: Arc<AppContext>,
    server: String,
    open_server: String,
    external_server: String,
    caller: AuthedFixture,
    owned: ContextId,
    _bootstrap: systemprompt_test_fixtures::TestBootstrap,
    _serve: tokio::task::JoinHandle<()>,
}

fn mcp_range_listener() -> anyhow::Result<std::net::TcpListener> {
    (5000..=5999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .ok_or_else(|| anyhow::anyhow!("no free port in the MCP range 5000-5999"))
}

async fn mount_backend(backend: &MockServer, server: &str) {
    let session = format!("sess-{server}");
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"clientInfo": {"name": format!("systemprompt-cli-{server}")}}
        })))
        .respond_with(move |request: &Request| {
            let body: serde_json::Value = request.body_json().expect("json body");
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", session.as_str())
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "contextless-stub", "version": "1.0.0"}
                    }
                }))
        })
        .mount(backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list"
        })))
        .respond_with(|request: &Request| {
            let body: serde_json::Value = request.body_json().expect("json body");
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {"tools": [{
                        "name": "echo",
                        "description": "Echo a message",
                        "inputSchema": {"type": "object", "properties": {}}
                    }]}
                }))
        })
        .mount(backend)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(backend)
        .await;
}

async fn mount_external_provider(backend: &MockServer) {
    Mock::given(method("POST"))
        .and(path(EXTERNAL_PATH))
        .respond_with(|request: &Request| {
            let body: serde_json::Value = request.body_json().expect("json body");
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0", "id": body["id"],
                    "result": {
                        "protocolVersion": "2025-11-25",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "external-stub", "version": "1.0.0"}
                    }
                }))
        })
        .mount(backend)
        .await;
}

const EXTERNAL_PATH: &str = "/external/mcp";

fn flavour_mcp_composition(extra: Option<DirectoryMember>) -> Vec<Box<dyn Extension>> {
    let keep = flavour_mcp_members();
    let mut members: Vec<Box<dyn Extension>> = fixture_core_members()
        .into_iter()
        .filter(|member| keep.contains(member.metadata().id))
        .collect();
    members.push(Box::new(systemprompt::mcp::host::McpHostExtension));
    members.push(Box::new(
        systemprompt_analytics_entry::extension::AnalyticsHttpExtension,
    ));
    let ids: BTreeSet<&str> = members.iter().map(|member| member.metadata().id).collect();
    assert_eq!(
        ids, keep,
        "the harness composes exactly the flavour-mcp members"
    );
    if let Some(member) = extra {
        members.push(Box::new(member));
    }
    members
}

async fn caller_credential(ctx: &AppContext) -> anyhow::Result<AuthedFixture> {
    let user_id = UserId::generate();
    let session_id = systemprompt_identifiers::SessionId::generate();
    let email = format!("contextless-{}@example.invalid", Uuid::new_v4().simple());
    seed_user_row_with_roles(
        ctx.db_pool(),
        &user_id,
        &email,
        &["admin".to_owned(), "user".to_owned()],
    )
    .await?;
    seed_user_session(ctx.db_pool(), &user_id, &session_id).await?;
    let jwt = JwtService::generate_admin_token(
        &systemprompt_test_fixtures::test_signing_keys(),
        &AdminTokenParams {
            user_id: &user_id,
            session_id: &session_id,
            email: &email,
            issuer: &ctx.config().jwt_issuer,
            duration: chrono::Duration::hours(1),
            client_id: None,
        },
    )?;
    Ok(AuthedFixture {
        user_id,
        session_id,
        email,
        jwt,
    })
}

async fn harness(directory: Directory) -> anyhow::Result<Harness> {
    let backend = MockServer::builder()
        .listener(mcp_range_listener()?)
        .start()
        .await;
    let open_backend = MockServer::builder()
        .listener(mcp_range_listener()?)
        .start()
        .await;
    let suffix = &Uuid::new_v4().simple().to_string()[..10];
    let server = format!("ctxless_{suffix}");
    let open_server = format!("ctxopen_{suffix}");
    let external_server = format!("ctxext_{suffix}");
    mount_backend(&backend, &server).await;
    mount_backend(&open_backend, &open_server).await;
    mount_external_provider(&backend).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let api = format!("http://{}", listener.local_addr()?);
    let yaml = format!(
        "mcp_servers:\n  {server}:\n    server_type: internal\n    binary: {server}-bin\n    port: {port}\n    enabled: true\n    tool_policy: allow\n    display_in_web: false\n    oauth:\n      required: true\n      scopes: [admin]\n      audience: mcp\n  {open_server}:\n    server_type: internal\n    binary: {open_server}-bin\n    port: {open_port}\n    enabled: true\n    tool_policy: allow\n    display_in_web: false\n    oauth:\n      required: false\n      scopes: []\n      audience: mcp\n  {external_server}:\n    server_type: external\n    endpoint: {backend_uri}{EXTERNAL_PATH}\n    enabled: true\n    tool_policy: allow\n    display_in_web: false\n    oauth:\n      required: true\n      scopes: [admin]\n      audience: mcp\n",
        port = backend.address().port(),
        open_port = open_backend.address().port(),
        backend_uri = backend.uri(),
    );
    let bootstrap = systemprompt_test_fixtures::bootstrap::init_isolated_bootstrap(&api, &yaml);
    for name in [&server, &open_server] {
        let manifest_dir = bootstrap.system_path.join("extensions").join(name);
        std::fs::create_dir_all(&manifest_dir)?;
        std::fs::write(
            manifest_dir.join("manifest.yaml"),
            format!("extension:\n  type: mcp\n  name: {name}\n  binary: {name}-bin\n"),
        )?;
    }

    let owner = UserId::generate();
    let owned = ContextId::generate();
    let extra = match directory {
        Directory::Absent => None,
        Directory::OwnsOnly | Directory::Fails | Directory::Withheld => Some(DirectoryMember {
            owner: owner.clone(),
            owned: owned.clone(),
            mode: directory,
        }),
    };
    let admitted = fixture_admitted_of(flavour_mcp_composition(extra));
    let ctx = systemprompt_test_fixtures::fixture_app_context_on_bootstrap(
        &bootstrap,
        bootstrap.boot_config().as_ref().clone(),
        admitted,
    )
    .await?;
    let services = ServiceRepository::new(ctx.db_pool(), ctx.config().instance_id.clone());
    for (name, port) in [
        (&server, backend.address().port()),
        (&open_server, open_backend.address().port()),
    ] {
        services
            .create_service(CreateServiceInput {
                name: &ServiceName::new(name.as_str()),
                module_name: ServiceModule::Mcp,
                status: ServiceStatus::Running,
                port,
                binary_mtime: None,
            })
            .await?;
    }
    let mut caller = caller_credential(&ctx).await?;
    if !matches!(directory, Directory::Absent) {
        caller = AuthedFixture {
            user_id: owner.clone(),
            ..caller
        };
        seed_user_row_with_roles(
            ctx.db_pool(),
            &owner,
            &format!("{owner}@example.invalid"),
            &["admin".to_owned(), "user".to_owned()],
        )
        .await?;
        seed_user_session(ctx.db_pool(), &owner, &caller.session_id).await?;
        caller.jwt = JwtService::generate_admin_token(
            &systemprompt_test_fixtures::test_signing_keys(),
            &AdminTokenParams {
                user_id: &owner,
                session_id: &caller.session_id,
                email: &caller.email,
                issuer: &ctx.config().jwt_issuer,
                duration: chrono::Duration::hours(1),
                client_id: None,
            },
        )?;
    }

    let router = systemprompt_api::services::server::setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let serve = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    Ok(Harness {
        api,
        backend,
        open_backend,
        ctx,
        server,
        open_server,
        external_server,
        caller,
        owned,
        _bootstrap: bootstrap,
        _serve: serve,
    })
}

fn contextless_session(caller: &AuthedFixture) -> CallerSession {
    CallerSession::new(
        SessionToken::new(caller.jwt.as_str()),
        caller.session_id.clone(),
        caller.user_id.clone(),
        None,
        UserType::Admin,
    )
}

async fn backend_posts(backend: &MockServer) -> Vec<Request> {
    backend
        .received_requests()
        .await
        .expect("recorded backend requests")
        .into_iter()
        .filter(|request| request.method.as_str() == "POST" && request.url.path() == "/mcp")
        .collect()
}

async fn external_posts(backend: &MockServer) -> Vec<Request> {
    backend
        .received_requests()
        .await
        .expect("recorded backend requests")
        .into_iter()
        .filter(|request| request.url.path() == EXTERNAL_PATH)
        .collect()
}

fn forwarded_context(h: &Harness, request: &Request) -> ContextId {
    let hop = request
        .headers
        .get(systemprompt_identifiers::headers::MCP_HOP)
        .and_then(|value| value.to_str().ok())
        .expect("the proxy attached a hop token");
    let claims = decode_mcp_hop(
        systemprompt_test_fixtures::test_signing_keys().keyring(),
        &h.ctx.config().jwt_issuer,
        &McpServerId::new(h.server.as_str()),
        hop,
    )
    .expect("the hop verifies");
    let signed = claims
        .identity
        .execution
        .expect("the hop signs the execution")
        .context_id;
    assert_eq!(
        request
            .headers
            .get(systemprompt_identifiers::headers::CONTEXT_ID)
            .and_then(|value| value.to_str().ok()),
        Some(signed.as_str()),
        "the forwarded context header is the one the hop signs"
    );
    signed
}

fn initialize(h: &Harness, context: &ContextId) -> reqwest::RequestBuilder {
    initialize_on(
        &h.api,
        &h.server,
        Some(h.caller.jwt.as_str()),
        Some(context.as_str()),
    )
}

fn initialize_on(
    api: &str,
    server: &str,
    bearer: Option<&str>,
    context: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = reqwest::Client::new()
        .post(format!("{api}/api/v1/mcp/{server}/mcp"))
        .header("accept", "application/json, text/event-stream");
    if let Some(bearer) = bearer {
        request = request.header("authorization", format!("Bearer {bearer}"));
    }
    if let Some(context) = context {
        request = request.header("x-context-id", context);
    }
    request.json(&serde_json::json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                   "clientInfo": {"name": format!("systemprompt-cli-{server}"), "version": "1"}}
    }))
}

#[tokio::test]
async fn flavour_mcp_admits_the_mcp_proxy_without_a_context_directory() -> anyhow::Result<()> {
    let h = harness(Directory::Absent).await?;
    let admitted = h.ctx.admitted().built();
    let keep = flavour_mcp_members();
    for id in reference().member_ids() {
        assert_eq!(
            admitted.contains(&ExtensionId::new(id)),
            keep.contains(id),
            "{id} is admitted exactly when flavour-mcp names it"
        );
    }
    for absent in ["agent", "agent_runs"] {
        assert!(!keep.contains(absent), "flavour-mcp has no {absent} member");
    }
    assert!(
        h.ctx
            .kernel()
            .state()
            .find::<DynContextDirectory>()
            .is_none(),
        "no member of flavour-mcp provides a context directory"
    );
    Ok(())
}

#[tokio::test]
async fn a_contextless_cli_session_initializes_and_lists_tools() -> anyhow::Result<()> {
    let h = harness(Directory::Absent).await?;
    let session = contextless_session(&h.caller);
    let target = McpTarget::through_proxy(&h.api, McpServerId::new(h.server.as_str()), &session);
    let tools = list_tools(&target, 20).await?;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        vec!["echo"]
    );
    let derived = ContextId::derived_from_session(&h.caller.session_id);
    let posts = backend_posts(&h.backend).await;
    assert!(!posts.is_empty(), "the backend saw the session");
    for request in &posts {
        assert_eq!(forwarded_context(&h, request), derived);
    }
    Ok(())
}

#[tokio::test]
async fn without_a_directory_a_named_context_runs_under_the_session_context() -> anyhow::Result<()>
{
    let h = harness(Directory::Absent).await?;
    let named = ContextId::generate();
    let response = initialize(&h, &named).send().await?;
    assert_eq!(response.status(), StatusCode::OK);
    let posts = backend_posts(&h.backend).await;
    let request = posts.first().expect("the backend saw the initialize");
    assert_eq!(
        forwarded_context(&h, request),
        ContextId::derived_from_session(&h.caller.session_id),
        "an unverifiable named context is not forwarded"
    );
    Ok(())
}

#[tokio::test]
async fn with_a_directory_a_context_the_caller_does_not_own_is_refused() -> anyhow::Result<()> {
    let h = harness(Directory::OwnsOnly).await?;
    let foreign = initialize(&h, &ContextId::generate()).send().await?;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert!(backend_posts(&h.backend).await.is_empty());

    let owned = initialize(&h, &h.owned).send().await?;
    assert_eq!(
        owned.status(),
        StatusCode::OK,
        "the owned context is admitted"
    );
    let posts = backend_posts(&h.backend).await;
    assert_eq!(
        forwarded_context(&h, posts.first().expect("the backend saw the call")),
        h.owned
    );
    Ok(())
}

#[tokio::test]
async fn with_a_directory_a_failed_lookup_is_refused() -> anyhow::Result<()> {
    let h = harness(Directory::Fails).await?;
    let response = initialize(&h, &h.owned).send().await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response.text().await?;
    assert!(body.contains("context_lookup_failed"), "{body}");
    assert!(backend_posts(&h.backend).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn with_a_directory_a_malformed_context_id_is_refused() -> anyhow::Result<()> {
    let h = harness(Directory::OwnsOnly).await?;
    let response = initialize_on(
        &h.api,
        &h.server,
        Some(h.caller.jwt.as_str()),
        Some("not-a-context"),
    )
    .send()
    .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response.text().await?;
    assert!(body.contains("context_not_owned"), "{body}");
    assert!(backend_posts(&h.backend).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn with_a_directory_an_unverified_caller_naming_a_context_is_refused() -> anyhow::Result<()> {
    let h = harness(Directory::OwnsOnly).await?;
    let owned = h.owned.as_str();
    let anonymous = initialize_on(&h.api, &h.open_server, None, Some(owned))
        .send()
        .await?;
    assert_eq!(anonymous.status(), StatusCode::FORBIDDEN);
    let body = anonymous.text().await?;
    assert!(body.contains("context_not_owned"), "{body}");

    let forged = initialize_on(
        &h.api,
        &h.open_server,
        Some("not-a-credential"),
        Some(owned),
    )
    .send()
    .await?;
    assert_eq!(
        forged.status(),
        StatusCode::UNAUTHORIZED,
        "an unverifiable bearer is refused at the edge"
    );
    assert!(backend_posts(&h.open_backend).await.is_empty());

    let verified = initialize_on(
        &h.api,
        &h.open_server,
        Some(h.caller.jwt.as_str()),
        Some(owned),
    )
    .send()
    .await?;
    assert_eq!(
        verified.status(),
        StatusCode::OK,
        "the verified owner is admitted"
    );
    assert_eq!(backend_posts(&h.open_backend).await.len(), 1);
    Ok(())
}

#[tokio::test]
async fn with_a_directory_an_external_server_refuses_a_foreign_context() -> anyhow::Result<()> {
    let h = harness(Directory::OwnsOnly).await?;
    let jwt = Some(h.caller.jwt.as_str());
    let foreign = ContextId::generate();
    let response = initialize_on(&h.api, &h.external_server, jwt, Some(foreign.as_str()))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response.text().await?;
    assert!(body.contains("context_not_owned"), "{body}");
    assert!(
        external_posts(&h.backend).await.is_empty(),
        "the provider was not reached"
    );

    let owned = initialize_on(&h.api, &h.external_server, jwt, Some(h.owned.as_str()))
        .send()
        .await?;
    assert_eq!(
        owned.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "the owned context passes the ownership check and stops at the missing bearer accessor"
    );
    assert!(external_posts(&h.backend).await.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_withheld_directory_refuses_a_named_context() -> anyhow::Result<()> {
    let h = harness(Directory::Withheld).await?;
    let state = h.ctx.kernel().state();
    assert!(state.find::<DynContextDirectory>().is_none());
    assert!(state.is_withheld::<DynContextDirectory>());

    for server in [&h.server, &h.external_server] {
        let response = initialize_on(
            &h.api,
            server,
            Some(h.caller.jwt.as_str()),
            Some(h.owned.as_str()),
        )
        .send()
        .await?;
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{server}"
        );
        let body = response.text().await?;
        assert!(body.contains("context_lookup_failed"), "{body}");
    }
    assert!(backend_posts(&h.backend).await.is_empty());
    assert!(external_posts(&h.backend).await.is_empty());

    let unnamed = initialize_on(&h.api, &h.server, Some(h.caller.jwt.as_str()), None)
        .send()
        .await?;
    assert_eq!(
        unnamed.status(),
        StatusCode::OK,
        "a request naming no context keeps its session's"
    );
    Ok(())
}
