//! Serialisation of first-run state for the webview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use super::state::FirstRunState;

#[derive(Serialize)]
pub(crate) struct FirstRunHostPayload<'a> {
    pub host_id: &'a str,
    pub display_name: &'a str,
    pub status: &'static str,
    pub error: Option<&'a str>,
}

#[derive(Serialize)]
pub(crate) struct FirstRunPayload<'a> {
    pub active: bool,
    pub done: bool,
    pub phase: &'static str,
    pub sync: &'static str,
    pub error: Option<&'a str>,
    pub hosts: Vec<FirstRunHostPayload<'a>>,
}

pub(crate) fn build(state: &FirstRunState) -> FirstRunPayload<'_> {
    FirstRunPayload {
        active: state.active,
        done: state.done,
        phase: state.phase.as_str(),
        sync: state.sync.as_str(),
        error: state.error.as_deref(),
        hosts: state
            .hosts
            .iter()
            .map(|h| FirstRunHostPayload {
                host_id: &h.host_id,
                display_name: &h.display_name,
                status: h.status.as_str(),
                error: h.error.as_deref(),
            })
            .collect(),
    }
}
