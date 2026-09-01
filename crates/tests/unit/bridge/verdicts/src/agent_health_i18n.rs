//! The verdict's serde codes ARE the localisation key suffixes — that is what
//! lets the renderer be a lookup with no branching on `profile_state`. The cost
//! is a silent coupling: a `rename_all` change or a new variant blanks the copy
//! for that state, because `t()` returns undefined on a miss and the UI shows
//! nothing rather than failing. So the coupling is pinned here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_bridge::integration::agent_health::{
    AgentAction, AgentReason, AgentState, FleetHeadline,
};
use systemprompt_bridge::integration::host_app::StaleReason;
use systemprompt_bridge::integration::proxy_probe::ProxyProbeState;

const FTL: &str = include_str!("../../../../../../bin/bridge/web/i18n/en-US/bridge.ftl");

// The kebab code the GUI will look up: a plain enum serialises to a string,
// a tagged one to an object carrying `code`.
fn code_of<T: Serialize>(v: &T) -> String {
    let json = serde_json::to_value(v).unwrap_or_default();
    match json {
        serde_json::Value::String(s) => s,
        serde_json::Value::Object(map) => map
            .get("code")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    }
}

fn assert_key(prefix: &str, code: &str) {
    assert!(
        !code.is_empty(),
        "`{prefix}` variant serialised without a code"
    );
    let key = format!("{prefix}-{code} =");
    assert!(
        FTL.lines().any(|l| l.starts_with(&key)),
        "no localisation entry `{prefix}-{code}` in bridge.ftl — this state would render blank"
    );
}

#[test]
fn every_agent_state_has_copy() {
    for s in [
        AgentState::Working,
        AgentState::Ready,
        AgentState::Attention,
        AgentState::NotSetUp,
        AgentState::Down,
        AgentState::Checking,
    ] {
        assert_key("agent-state", &code_of(&s));
    }
}

#[test]
fn every_agent_reason_has_copy() {
    for r in [
        AgentReason::Governed { when_unix: None },
        AgentReason::Awaiting,
        AgentReason::AppMissing,
        AgentReason::Stale {
            cause: StaleReason::LoopbackSecret,
        },
        AgentReason::Partial {
            missing: String::new(),
        },
        AgentReason::Absent,
        AgentReason::NoKey {
            providers: String::new(),
        },
        AgentReason::NoModels,
        AgentReason::ProxyDown {
            probe: ProxyProbeState::Refused,
        },
        AgentReason::NeverProbed,
        AgentReason::CloudManaged,
    ] {
        assert_key("agent-reason", &code_of(&r));
    }
}

#[test]
fn every_agent_action_has_copy() {
    for a in [
        AgentAction::Download,
        AgentAction::Repair,
        AgentAction::Verify,
        AgentAction::Open,
        AgentAction::Add,
    ] {
        assert_key("agent-action", &code_of(&a));
    }
}

#[test]
fn every_fleet_headline_has_copy() {
    for h in [
        FleetHeadline::AllWorking,
        FleetHeadline::NeedsAttention,
        FleetHeadline::NotWorking,
        FleetHeadline::Checking,
        FleetHeadline::NoneEnabled,
    ] {
        assert_key("agents-fleet", &code_of(&h));
    }
}
