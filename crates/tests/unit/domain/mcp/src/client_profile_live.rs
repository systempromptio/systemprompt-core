//! Derives a client profile from a live rmcp request context.
//!
//! `client_profile_from_peer` reads what the peer negotiated at `initialize`.
//! The only honest way to test that is to negotiate: an in-process duplex
//! carries a real handshake, and the server records the profile it derives
//! from inside `call_tool`.

use std::sync::{Arc, Mutex};

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ClientCapabilities, ClientInfo,
    ErrorData, Implementation, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, ServiceExt};
use systemprompt_mcp::client_profile_from_peer;
use systemprompt_mcp::services::client::McpClientHandler;
use systemprompt_models::mcp::ClientProfile;

#[derive(Debug, Clone, Default)]
struct ProfileRecorder {
    seen: Arc<Mutex<Option<ClientProfile>>>,
}

impl ServerHandler for ProfileRecorder {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        *self.seen.lock().expect("recorder lock") = Some(client_profile_from_peer(&context));
        Ok(CallToolResponse::Complete(CallToolResult::success(vec![])))
    }
}

async fn profile_seen_by_server(capabilities: ClientCapabilities, name: &str) -> ClientProfile {
    let recorder = ProfileRecorder::default();
    let seen = recorder.seen.clone();

    let (client_side, server_side) = tokio::io::duplex(16 * 1024);
    let server_task = tokio::spawn(async move { recorder.serve(server_side).await });

    let handler = McpClientHandler::new(ClientInfo::new(
        capabilities,
        Implementation::new(name, "1.0.0"),
    ));
    let client = handler.serve(client_side).await.expect("client handshake");

    client
        .call_tool(CallToolRequestParams::new("probe".to_owned()))
        .await
        .expect("tool call reaches the recorder");

    let server = server_task
        .await
        .expect("server task joins")
        .expect("server handshake");

    let profile = seen
        .lock()
        .expect("recorder lock")
        .clone()
        .expect("call_tool recorded a profile");

    let _ = client.cancel().await;
    let _ = server.cancel().await;
    profile
}

fn capabilities_with_ui() -> ClientCapabilities {
    let mut caps = ClientCapabilities::default();
    let mut extensions = std::collections::BTreeMap::new();
    extensions.insert(
        "io.modelcontextprotocol/ui".to_owned(),
        serde_json::Map::new(),
    );
    caps.extensions = Some(extensions);
    caps
}

// Why: structured content is gated on the negotiated protocol version, not on
// an announced extension. A live handshake settles on a modern version, so the
// richer wire is available even to a client that announced nothing.
#[tokio::test]
async fn the_live_peers_name_and_protocol_reach_the_profile() {
    let profile = profile_seen_by_server(ClientCapabilities::default(), "cowork").await;

    assert_eq!(
        profile.client_name.as_deref(),
        Some("cowork"),
        "the profile must name the client that actually connected"
    );
    assert!(
        profile.protocol_version.is_some(),
        "a negotiated request context always carries a protocol version"
    );
    assert!(
        profile.supports_structured_content(),
        "the negotiated version is modern, so structured content is available: {profile:?}"
    );
}

// Why: the extension list decides what the response builder may put on the
// wire. Losing it here serves a UI-capable client the plain-text wire.
#[tokio::test]
async fn an_extension_the_live_peer_announced_is_carried_into_the_profile() {
    let profile = profile_seen_by_server(capabilities_with_ui(), "ui-client").await;

    assert!(
        profile.supports_ui(),
        "a client that announced the UI extension must be recognised: {profile:?}"
    );
    assert!(
        profile
            .extensions
            .iter()
            .any(|e| e == "io.modelcontextprotocol/ui"),
        "the announced extension key must survive verbatim: {:?}",
        profile.extensions
    );
}

// Why: the converse. A client that announced nothing must not acquire
// capabilities by default — the profile is what gates the richer wire.
#[tokio::test]
async fn a_live_peer_that_announced_no_extensions_gets_none() {
    let profile = profile_seen_by_server(ClientCapabilities::default(), "plain").await;

    assert!(
        profile.extensions.is_empty(),
        "no extensions were announced: {:?}",
        profile.extensions
    );
    assert!(!profile.supports_ui());
}
