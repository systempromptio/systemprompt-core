use std::sync::{Arc, OnceLock};

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp_host::HostContext;
use systemprompt_mcp_host::toolsets::{
    DOCUMENTS_TOOLSET, ISSUES_TOOLSET, McpHostError, build_server, registered_toolset_ids, serve,
    serve_on,
};
use systemprompt_test_fixtures::{TestBootstrap, host_child_env, init_services_bootstrap};

const SERVICES: &str = "mcp_servers:
  host_full:
    server_type: internal
    binary: host-full
    package: null
    port: 5393
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
    toolsets:
      - id: documents
  host_issues:
    server_type: internal
    binary: host-issues
    package: null
    port: 5395
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
    toolsets:
      - id: issues
  host_ghost:
    server_type: internal
    binary: host-ghost
    package: null
    port: 5394
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
    toolsets:
      - id: no_such_toolset
  host_portless:
    server_type: external
    endpoint: http://127.0.0.1:1/mcp
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
";

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

async fn ctx() -> Arc<HostContext> {
    let bootstrap = BOOT.get_or_init(|| {
        let bootstrap = init_services_bootstrap(SERVICES);
        std::fs::write(
            bootstrap.services_path.join("config/files.yaml"),
            "files:\n  documents:\n    enabled: true\n",
        )
        .expect("write files.yaml");
        bootstrap
    });
    let env = host_child_env(bootstrap);
    systemprompt_test_fixtures::seed_fixture_system_admin(
        &systemprompt_test_fixtures::test_db_pool().await,
    )
    .await
    .expect("seed the system admin");
    Arc::new(
        HostContext::boot(move |name| env.get(name).cloned())
            .await
            .expect("host context from the child environment"),
    )
}

#[test]
fn core_registers_the_issues_and_documents_toolsets() {
    let ids = registered_toolset_ids();
    assert!(ids.contains(&ISSUES_TOOLSET), "{ids:?}");
    assert!(ids.contains(&DOCUMENTS_TOOLSET), "{ids:?}");
}

#[tokio::test]
async fn a_declared_server_hosts_every_tool_of_its_toolsets() {
    let ctx = ctx().await;
    let server = build_server(&ctx, &McpServerId::new("host_full")).expect("server");
    assert_eq!(server.server_id().as_str(), "host_full");
    let names = server.tool_names();
    assert!(!names.contains(&"report_issue"), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("document")), "{names:?}");
    let info = rmcp::ServerHandler::get_info(&server);
    assert!(info.instructions.is_some());
}

#[tokio::test]
async fn a_toolset_disabled_in_this_installation_fails_the_build() {
    let ctx = ctx().await;
    let err = build_server(&ctx, &McpServerId::new("host_issues")).expect_err("disabled");
    assert!(
        matches!(err, McpHostError::Build { ref toolset, .. } if toolset == ISSUES_TOOLSET),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_undeclared_server_is_refused() {
    let ctx = ctx().await;
    let err = build_server(&ctx, &McpServerId::new("host_absent")).expect_err("absent");
    assert!(
        matches!(err, McpHostError::UnknownServer { ref server } if server.as_str() == "host_absent")
    );
}

#[tokio::test]
async fn a_toolset_no_crate_registers_is_refused() {
    let ctx = ctx().await;
    let err = build_server(&ctx, &McpServerId::new("host_ghost")).expect_err("ghost");
    match err {
        McpHostError::UnknownToolset {
            toolset,
            registered,
            ..
        } => {
            assert_eq!(toolset, "no_such_toolset");
            assert!(registered.contains(ISSUES_TOOLSET), "{registered}");
        },
        other => panic!("expected UnknownToolset, got {other:?}"),
    }
}

#[tokio::test]
async fn serving_a_server_without_a_port_is_refused() {
    let ctx = ctx().await;
    let err = serve(Arc::clone(&ctx), McpServerId::new("host_portless"))
        .await
        .expect_err("no port");
    assert!(
        matches!(err, McpHostError::NoPort { ref server } if server.as_str() == "host_portless")
    );
    let err = serve(ctx, McpServerId::new("host_absent"))
        .await
        .expect_err("absent");
    assert!(matches!(err, McpHostError::NoPort { .. }));
}

#[tokio::test]
async fn serve_on_refuses_an_unbuildable_server_without_serving() {
    let ctx = ctx().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let err = serve_on(ctx, McpServerId::new("host_ghost"), listener)
        .await
        .expect_err("ghost");
    assert!(matches!(err, McpHostError::UnknownToolset { .. }));
}

#[tokio::test]
async fn serve_on_answers_http_on_the_listener() {
    let ctx = ctx().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let task = tokio::spawn(serve_on(ctx, McpServerId::new("host_full"), listener));
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/mcp"))
        .send()
        .await
        .expect("http answer");
    assert!(
        !response.status().is_server_error(),
        "{}",
        response.status()
    );
    assert!(!task.is_finished());
    task.abort();
}

fn non_loopback_interface() -> std::net::IpAddr {
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").expect("udp bind");
    probe
        .connect("192.0.2.1:9")
        .expect("route to a non-loopback address");
    let ip = probe.local_addr().expect("local addr").ip();
    assert!(
        !ip.is_loopback(),
        "the host has a non-loopback interface: {ip}"
    );
    ip
}

#[tokio::test]
async fn the_host_listener_is_not_reachable_on_a_non_loopback_address() {
    let listener = systemprompt_mcp::bind_loopback(0).await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    assert!(addr.ip().is_loopback(), "{addr}");

    let local = tokio::net::TcpStream::connect(addr).await;
    assert!(local.is_ok(), "loopback reaches the host");

    let external = non_loopback_interface();
    let refused = tokio::net::TcpStream::connect((external, addr.port())).await;
    assert!(
        refused.is_err(),
        "{external}:{} must not reach the host",
        addr.port()
    );
}
