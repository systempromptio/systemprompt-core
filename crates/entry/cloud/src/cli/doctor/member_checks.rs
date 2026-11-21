//! `cloud doctor`'s contributed checks: every admitted member's
//! [`DoctorCheck`](systemprompt_traits::DoctorCheck), core and install
//! alike, run in composition order and reported under the check's own name.
//!
//! Every check gets the same [`DoctorContext`]: the presence of each secret,
//! never its value, and [`TenantProbes`], the read-only facts the doctor
//! answers itself. The probes read through one short-lived pool opened from
//! the checked profile's secrets with `default_transaction_read_only` on;
//! no check receives the pool. With no admitted composition the member checks
//! are not evaluated.
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
use systemprompt_database::Database;
use systemprompt_marketplace::managed::ManagedRepository;
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

struct TenantProbes {
    managed: Option<ManagedRepository>,
}

#[async_trait]
impl DoctorProbes for TenantProbes {
    async fn git_credential_sources(&self) -> Result<Vec<GitCredentialSource>, DoctorProbeError> {
        let Some(managed) = &self.managed else {
            return Err(DoctorProbeError::Unreachable);
        };
        managed
            .git_sources_with_credentials()
            .await
            .map_err(|error| DoctorProbeError::Read(Box::new(error)))
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
    let probes = TenantProbes {
        managed: pool.as_ref().map(|pool| {
            ManagedRepository::new(&Arc::new(Database::from_pools(Arc::clone(pool), None)))
        }),
    };
    let ctx = DoctorContext {
        secrets,
        probes: &probes,
    };
    let results = views
        .doctor
        .run(ctx)
        .await
        .into_iter()
        .map(|(owned, outcome)| CheckResult {
            name: owned.check.name(),
            status: status(outcome.status),
            detail: outcome.detail,
        })
        .collect();
    drop(probes);
    if let Some(pool) = pool {
        pool.close().await;
    }
    results
}
