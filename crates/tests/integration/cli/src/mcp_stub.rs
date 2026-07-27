//! Scripted streamable-HTTP MCP endpoint for subprocess tests.
//!
//! [`stub_port`] lazily starts a wiremock server on an ephemeral port that
//! answers the MCP handshake, `tools/list`, and `tools/call`, then rewrites
//! the shared fixture's services config so the fixture MCP server points at it
//! and upserts a `running` services row so `resolve_running_port` finds it.
//!
//! The server name is per-process ([`fixture_mcp_server`]) so concurrent test
//! processes cannot repoint each other's `services` row.

use std::sync::OnceLock;

use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::full_bootstrap::{database_url, fixture, fixture_mcp_server, rewrite_services_config};

static STUB: OnceLock<Option<u16>> = OnceLock::new();

pub fn stub_port() -> Option<u16> {
    *STUB.get_or_init(|| {
        let fixture = fixture()?;
        let url = database_url()?;
        let port = start_stub_server();
        rewrite_services_config(fixture, port);
        register_running_service(&url, port);
        Some(port)
    })
}

fn start_stub_server() -> u16 {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build stub runtime");
        runtime.block_on(async move {
            let server = MockServer::start().await;
            mount_mcp_mocks(&server).await;
            tx.send(server.address().port()).expect("send stub port");
            std::future::pending::<()>().await;
        });
    });
    rx.recv().expect("receive stub port")
}

fn register_running_service(database_url: &str, port: u16) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build registration runtime");
    runtime.block_on(async {
        let pool = sqlx::PgPool::connect(database_url)
            .await
            .expect("connect to test database");
        // The row carries this test process's pid because
        // `ServiceRepository::cleanup_stale_entries` deletes every
        // `status = 'running' AND pid IS NULL` row across the whole table, and
        // a concurrently-running test process boots an orchestrator that calls
        // it. A pid-less stub registration was therefore deleted out from under
        // this process, and the command under test saw no running MCP server.
        // The pid is honest: the wiremock stub is hosted by this process.
        let pid = i32::try_from(std::process::id()).expect("pid fits in i32");
        sqlx::query(
            "INSERT INTO services (name, module_name, server_type, port, status, pid)
             VALUES ($1, 'mcp', 'external', $2, 'running', $3)
             ON CONFLICT (name) DO UPDATE SET port = $2, status = 'running', pid = $3",
        )
        .bind(fixture_mcp_server())
        .bind(i32::from(port))
        .bind(pid)
        .execute(&pool)
        .await
        .expect("register running fixture_mcp service");
    });
}

async fn mount_mcp_mocks(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            serde_json::json!({"method": "initialize"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .insert_header("mcp-session-id", "sess-cli-stub")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "result": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "cli-stub", "version": "1.0.0"}
                    }
                })),
        )
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "notifications/initialized"
        })))
        .respond_with(ResponseTemplate::new(202))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/list"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"tools": [
                        {
                            "name": "echo",
                            "description": "Echo a message",
                            "inputSchema": {
                                "type": "object",
                                "properties": {"message": {"type": "string"}},
                                "required": ["message"]
                            }
                        },
                        {
                            "name": "boom",
                            "description": "Always reports a tool error",
                            "inputSchema": {"type": "object"}
                        }
                    ]}
                })),
        )
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call",
            "params": {"name": "boom"}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "content": [{"type": "text", "text": "boom exploded"}],
                        "isError": true
                    }
                })),
        )
        .with_priority(1)
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call",
            "params": {"name": "reject"}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": {"code": -32602, "message": "unknown tool: reject"}
                })),
        )
        .with_priority(1)
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(serde_json::json!({
            "method": "tools/call"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {
                        "content": [{"type": "text", "text": "stub output"}],
                        "isError": false
                    }
                })),
        )
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}
