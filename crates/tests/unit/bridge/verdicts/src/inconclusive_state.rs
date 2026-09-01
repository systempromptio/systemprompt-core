//! Guards on the two mistakes that made the bridge report false alarms:
//! an inconclusive probe treated as a definite failure, and a UI literal that
//! no longer matched the enum variant it was written against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_bridge::proxy::mcp_probe::McpAuthState;

// The catalogue keys `mcp-auth-<code>` are looked up by the serialised name,
// so the wire spelling is the contract. It once tested for `"Ok"`, which never
// existed, so every healthy server matched the "needs signing in" filter.
#[test]
fn mcp_auth_states_serialize_as_kebab_codes() {
    let name = |s: McpAuthState| serde_json::to_string(&s).unwrap_or_default();
    assert_eq!(name(McpAuthState::Authenticated), "\"authenticated\"");
    assert_eq!(
        name(McpAuthState::GatewayUnauthorized),
        "\"gateway-unauthorized\""
    );
    assert_eq!(name(McpAuthState::NotRegistered), "\"not-registered\"");
    assert_eq!(name(McpAuthState::ProbeTimeout), "\"probe-timeout\"");
    assert_eq!(name(McpAuthState::LocalError), "\"local-error\"");
    assert_eq!(name(McpAuthState::Unknown), "\"unknown\"");
}

#[test]
fn only_an_auth_rejection_asks_the_user_to_sign_in() {
    assert!(McpAuthState::GatewayUnauthorized.needs_sign_in());
    assert!(McpAuthState::NotRegistered.needs_sign_in());

    for state in [
        McpAuthState::Authenticated,
        McpAuthState::Unknown,
        McpAuthState::NoServers,
        McpAuthState::ProxyUnreachable,
        McpAuthState::ProbeTimeout,
        McpAuthState::LocalError,
        McpAuthState::ProtocolError,
        McpAuthState::UpstreamError,
        McpAuthState::LoopbackMismatch,
    ] {
        assert!(
            !state.needs_sign_in(),
            "{state:?} must not be reported as needing a sign-in"
        );
    }
}

// A result that establishes nothing must never replace one that does --
// this is what `AppState::apply_mcp_auth` keys its merge on.
#[test]
fn a_probe_that_reached_no_verdict_is_not_conclusive() {
    for state in [
        McpAuthState::Unknown,
        McpAuthState::ProxyUnreachable,
        McpAuthState::ProbeTimeout,
        McpAuthState::LocalError,
    ] {
        assert!(!state.is_conclusive(), "{state:?} concluded nothing");
    }

    for state in [
        McpAuthState::Authenticated,
        McpAuthState::NoServers,
        McpAuthState::GatewayUnauthorized,
        McpAuthState::NotRegistered,
        McpAuthState::LoopbackMismatch,
        McpAuthState::UpstreamError,
        McpAuthState::ProtocolError,
    ] {
        assert!(state.is_conclusive(), "{state:?} is a real finding");
    }
}

// The GUI is compiled only where it ships, so this guard travels with it.
#[cfg(any(target_os = "windows", target_os = "macos"))]
use systemprompt_bridge::gui::error::GuiError;

// Cancellation used to be detected by searching the error message for a
// phrase. Two call sites spelled it differently and the check silently stopped
// matching, so pressing Cancel on sign-in reported "unauthorized".
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[test]
fn cancellation_is_recognised_by_type_not_by_message() {
    assert!(GuiError::Cancelled.is_cancelled());
    assert!(GuiError::Auth(systemprompt_bridge::auth::setup::SetupError::Cancelled).is_cancelled());

    assert!(!GuiError::NotAuthenticated.is_cancelled());
    assert!(
        !GuiError::Auth(systemprompt_bridge::auth::setup::SetupError::Io(
            "sign-in cancelled".to_owned()
        ))
        .is_cancelled(),
        "a message that merely reads as cancelled is not a cancellation"
    );
}
