//! The admitted extensions' readiness checks, run by `/readyz` and
//! `/health/detail`; a check that fails, errors or overruns the probe
//! timeout makes the answer unready.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::json;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{ReadinessCheck, ReadinessFailure, SlotError};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::AppContext;

use super::contributed::admitted_factories;
use super::health::HEALTH_PROBE_TIMEOUT;

/// The enabled extensions' readiness checks, run by `/readyz` and
/// `/health/detail`.
pub struct ExtensionReadiness {
    checks: Vec<(ExtensionId, Arc<dyn ReadinessCheck>)>,
}

impl std::fmt::Debug for ExtensionReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionReadiness").finish_non_exhaustive()
    }
}

impl ExtensionReadiness {
    pub(crate) fn from_context(ctx: &AppContext) -> Result<Self, SlotError> {
        Ok(Self {
            checks: admitted_factories(ctx, "readiness", ContributionSink::readiness)?,
        })
    }

    // JSON: the per-check `/readyz` entries are an open JSON array.
    pub(crate) async fn run(&self) -> (bool, serde_json::Value) {
        let outcomes = futures_util::future::join_all(self.checks.iter().map(
            |(extension, check)| async move {
                let outcome = tokio::time::timeout(HEALTH_PROBE_TIMEOUT, check.check()).await;
                check_entry(extension.as_str(), check.name(), outcome)
            },
        ))
        .await;
        let ready = outcomes.iter().all(|(ok, _)| *ok);
        let entries = outcomes.into_iter().map(|(_, entry)| entry).collect();
        (ready, serde_json::Value::Array(entries))
    }
}

pub(crate) fn readiness_layer(
    ctx: &AppContext,
) -> Result<axum::Extension<Arc<ExtensionReadiness>>, SlotError> {
    Ok(axum::Extension(Arc::new(ExtensionReadiness::from_context(
        ctx,
    )?)))
}

// JSON: one entry of the outgoing `/readyz` extensions array.
fn check_entry(
    extension: &str,
    check: &'static str,
    outcome: Result<Result<(), ReadinessFailure>, tokio::time::error::Elapsed>,
) -> (bool, serde_json::Value) {
    match outcome {
        Ok(Ok(())) => (
            true,
            json!({ "extension": extension, "check": check, "status": "ok" }),
        ),
        Ok(Err(ReadinessFailure::Unready { reason })) => (
            false,
            json!({ "extension": extension, "check": check, "status": "unready", "reason": reason }),
        ),
        Ok(Err(error)) => {
            tracing::warn!(extension, check, error = %error, "extension readiness check errored");
            (
                false,
                json!({ "extension": extension, "check": check, "status": "error" }),
            )
        },
        Err(_) => (
            false,
            json!({ "extension": extension, "check": check, "status": "timeout" }),
        ),
    }
}
