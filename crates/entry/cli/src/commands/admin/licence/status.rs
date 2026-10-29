//! `admin licence status`: whether this deployment checks in, and the last
//! attempt, success and error recorded by the `licence_checkin` job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
use systemprompt_identifiers::EnterpriseLicenseId;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::LicenceCheckinRepository;

use crate::shared::CommandOutput;

#[derive(Debug, Serialize)]
struct LicenceStatusOutput {
    enabled: bool,
    licence_id: Option<EnterpriseLicenseId>,
    endpoint: String,
    interval_secs: u64,
    last_attempt_at: Option<String>,
    last_success_at: Option<String>,
    last_status: Option<i32>,
    last_error: Option<String>,
    last_error_at: Option<String>,
    attempts_total: i64,
    failures_total: i64,
}

pub(super) async fn execute(app: &Arc<AppContext>) -> Result<CommandOutput> {
    let licence = &app.profile().licence;
    let repository = LicenceCheckinRepository::new(app.db_pool().pool().as_ref().clone());
    let state = repository.find_state().await?.unwrap_or_default();
    let output = LicenceStatusOutput {
        enabled: licence.enabled(),
        licence_id: licence.licence_id.clone(),
        endpoint: licence.endpoint.clone(),
        interval_secs: licence.effective_interval_secs(),
        last_attempt_at: state.last_attempt_at.map(|t| t.to_rfc3339()),
        last_success_at: state.last_success_at.map(|t| t.to_rfc3339()),
        last_status: state.last_status,
        last_error: state.last_error,
        last_error_at: state.last_error_at.map(|t| t.to_rfc3339()),
        attempts_total: state.attempts_total,
        failures_total: state.failures_total,
    };
    Ok(CommandOutput::card_value("Licence check-in", &output))
}
