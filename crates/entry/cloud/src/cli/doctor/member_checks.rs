//! `cloud doctor`'s contributed checks: every admitted member's
//! [`DoctorCheck`](systemprompt_traits::DoctorCheck), core and install
//! alike, run in composition order and reported under the check's own name.
//!
//! Every check gets a [`DoctorContext`]: the presence of each secret, never
//! its value, and read-only probes. A core member's check reads the probes
//! the admitted core probe source (`DoctorProbeSource`) opens over one
//! short-lived pool opened from the checked profile's secrets with
//! `default_transaction_read_only` on; no check receives the pool, and with
//! no source or no reachable database the core probes answer `Unreachable`. An
//! install member's check reads [`WithheldProbes`]: the tenant's Git source
//! list (repository URLs included) is not a fact an install check is told, so
//! every probe answers `Unreachable`. With no admitted composition the member
//! checks are not evaluated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use systemprompt_extension::contribute::StandingTag;
use systemprompt_traits::{
    DoctorContext, DoctorProbeError, DoctorProbes, DoctorStatus, GitCredentialSource,
};

use super::{CheckResult, CheckStatus, Composition, NEEDS_COMPOSITION};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

async fn connect_read_only(secrets: &HashMap<String, String>) -> Option<Arc<PgPool>> {
    let url = secrets
        .get("external_database_url")
        .or_else(|| secrets.get("database_url"))?;
    let options = PgConnectOptions::from_str(url)
        .inspect_err(|error| tracing::debug!(%error, "doctor database url unparseable"))
        .ok()?
        .options([("default_transaction_read_only", "on")]);
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect_with(options)
        .await
        .inspect_err(|error| tracing::debug!(%error, "doctor database unreachable"))
        .ok()
        .map(Arc::new)
}

struct WithheldProbes;

#[async_trait]
impl DoctorProbes for WithheldProbes {
    async fn git_credential_sources(&self) -> Result<Vec<GitCredentialSource>, DoctorProbeError> {
        Err(DoctorProbeError::Unreachable)
    }
}

const fn status(status: DoctorStatus) -> CheckStatus {
    match status {
        DoctorStatus::Pass => CheckStatus::Pass,
        DoctorStatus::Warn => CheckStatus::Warn,
        DoctorStatus::Fail => CheckStatus::Fail,
    }
}

pub(super) async fn run_member_checks(
    secrets: &HashMap<String, String>,
    views: Composition<'_>,
) -> Vec<CheckResult> {
    let Ok(views) = views else {
        return vec![CheckResult::not_evaluated(
            "member-checks",
            NEEDS_COMPOSITION,
        )];
    };
    if views.doctor.entries().is_empty() {
        return Vec::new();
    }
    let pool = connect_read_only(secrets).await;
    let probes: Box<dyn DoctorProbes> = match (pool.as_ref(), views.doctor.probe_source()) {
        (Some(pool), Some(source)) => source.probes(Arc::clone(pool)),
        _ => Box::new(WithheldProbes),
    };
    let core = DoctorContext {
        secrets,
        probes: probes.as_ref(),
    };
    let install = DoctorContext {
        secrets,
        probes: &WithheldProbes,
    };
    let mut results = Vec::with_capacity(views.doctor.entries().len());
    for owned in views.doctor.entries() {
        let ctx = if owned.standing == StandingTag::Core {
            core
        } else {
            install
        };
        let outcome = owned.check.check(ctx).await;
        results.push(CheckResult {
            name: owned.check.name(),
            status: status(outcome.status),
            detail: outcome.detail,
        });
    }
    drop(probes);
    if let Some(pool) = pool {
        pool.close().await;
    }
    results
}
