//! MCP auth probe results land in application state per server: a probe of a
//! slug the registry does not know leaves every recorded server untouched,
//! and an inconclusive fresh result never overwrites a conclusive prior one.

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gui::state::AppState;
use systemprompt_bridge::proxy::mcp_probe::{McpAuthState, McpServerAuth};

fn state() -> std::sync::Arc<AppState> {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    AppState::new_loaded(ctx)
}

fn auth(id: &str, state: McpAuthState) -> McpServerAuth {
    McpServerAuth {
        id: id.to_owned(),
        url: format!("http://127.0.0.1:48217/mcp/{id}"),
        state,
        tools: Vec::new(),
        http_status: None,
        latency_ms: None,
        error: None,
        session_id: None,
        probed_at_unix: 1_753_948_800,
    }
}

#[test]
fn a_probe_of_an_unknown_slug_leaves_the_recorded_servers_untouched() {
    let state = state();
    state.apply_mcp_auth(vec![
        auth("odoo", McpAuthState::Authenticated),
        auth("knowledge-bank", McpAuthState::GatewayUnauthorized),
    ]);
    assert!(state.mark_mcp_auth_probing());
    assert!(
        !state.mark_mcp_auth_probing(),
        "a second probe is refused while one is in flight"
    );

    state.finish_mcp_auth_probe();

    let snap = state.snapshot();
    assert!(!snap.mcp_auth_probe_in_flight);
    assert_eq!(snap.mcp_auth.len(), 2);
    assert_eq!(snap.mcp_auth[0].state, McpAuthState::Authenticated);
    assert_eq!(snap.mcp_auth[1].state, McpAuthState::GatewayUnauthorized);
    assert!(
        state.mark_mcp_auth_probing(),
        "the probe slot is free again"
    );
}

#[test]
fn an_inconclusive_single_result_keeps_a_conclusive_prior_verdict() {
    let state = state();
    state.apply_mcp_auth(vec![auth("odoo", McpAuthState::Authenticated)]);

    state.apply_mcp_auth_one(auth("odoo", McpAuthState::ProbeTimeout));
    assert_eq!(
        state.snapshot().mcp_auth[0].state,
        McpAuthState::Authenticated,
        "a timeout says nothing about the server; the last real verdict stands"
    );

    state.apply_mcp_auth_one(auth("odoo", McpAuthState::NotRegistered));
    assert_eq!(
        state.snapshot().mcp_auth[0].state,
        McpAuthState::NotRegistered,
        "a conclusive result replaces the prior one"
    );

    state.apply_mcp_auth_one(auth("new-server", McpAuthState::ProbeTimeout));
    let snap = state.snapshot();
    assert_eq!(snap.mcp_auth.len(), 2);
    assert_eq!(
        snap.mcp_auth[1].state,
        McpAuthState::ProbeTimeout,
        "a server with no prior verdict records whatever the probe found"
    );
}
