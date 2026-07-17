//! End-of-run finalization for JSON/YAML output.
//!
//! When a command runs in structured-output mode but emits no artifact of its
//! own, the levelled notices it logged are flushed as a single `message`
//! artifact so stdout always carries exactly one parseable [`CliArtifact`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;

use systemprompt_logging::{
    CliService, drain_notices, is_structured_output, structured_was_emitted,
};
use systemprompt_models::artifacts::{CliArtifact, MessageArtifact, NoticeLine};

pub(super) fn finalize(outcome: &Result<()>) {
    if outcome.is_err() || !is_structured_output() || structured_was_emitted() {
        return;
    }

    let mut lines: Vec<NoticeLine> = drain_notices()
        .into_iter()
        .map(|n| NoticeLine::new(n.level, n.text))
        .collect();
    if lines.is_empty() {
        lines.push(NoticeLine::new("success", "Command completed."));
    }
    CliService::json(&CliArtifact::message(MessageArtifact::new(lines)));
}
