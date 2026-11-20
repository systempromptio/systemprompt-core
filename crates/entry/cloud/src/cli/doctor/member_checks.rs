//! `cloud doctor`'s contributed checks: every admitted member's
//! [`DoctorCheck`](systemprompt_traits::DoctorCheck), run in composition
//! order and reported under the check's own name.
//!
//! The doctor opens one short-lived database pool from the checked
//! profile's secrets (when its database is reachable) and hands each check
//! that pool and the presence of each secret, never a secret's value. With
//! no admitted composition the member checks are not evaluated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use systemprompt_database::Database;
use systemprompt_traits::{Dependencies, DoctorContext, DoctorStatus};

use super::{CheckResult, CheckStatus, Composition, NEEDS_COMPOSITION};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

async fn connect(secrets: &HashMap<String, String>) -> Option<Arc<PgPool>> {
    let url = secrets
        .get("external_database_url")
        .or_else(|| secrets.get("database_url"))?;
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect(url)
        .await
        .inspect_err(|error| tracing::debug!(%error, "doctor database unreachable"))
        .ok()
        .map(Arc::new)
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
    let checks = views.doctor.entries();
    if checks.is_empty() {
        return Vec::new();
    }
    let pool = connect(secrets).await;
    let mut handles = Dependencies::new();
    if let Some(pool) = &pool {
        handles.insert(Arc::new(Database::from_pools(Arc::clone(pool), None)));
    }
    let ctx = DoctorContext {
        secrets,
        handles: &handles,
    };
    let mut results = Vec::with_capacity(checks.len());
    for owned in checks {
        let outcome = owned.check.check(ctx).await;
        results.push(CheckResult {
            name: owned.check.name(),
            status: status(outcome.status),
            detail: outcome.detail,
        });
    }
    drop(handles);
    if let Some(pool) = pool {
        pool.close().await;
    }
    results
}
