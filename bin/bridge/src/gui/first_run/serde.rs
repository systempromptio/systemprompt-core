//! Builds the first-run wire payload from the GUI's first-run state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::state::FirstRunState;
pub(crate) use crate::wire::first_run::{FirstRunHostPayload, FirstRunPayload};

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
