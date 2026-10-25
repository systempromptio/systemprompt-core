//! `cloud doctor`'s config-plane check: the profile's `unapplied_planes` mode
//! and whether `config apply` has recorded each runtime plane on the target
//! database.
//!
//! An unapplied plane is a warning, never a blocking failure: the deploy runs
//! `config apply` itself. The detail names the mode, so an operator reading a
//! `warn` profile knows the node serves those planes ungoverned until then.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use systemprompt_database::{PLANE_NOT_APPLIED_HINT, find_config_plane_applied};
use systemprompt_gateway::GATEWAY_POLICIES_PLANE;
use systemprompt_governance::authz::ACCESS_CONTROL_PLANE;
use systemprompt_governance::policy::GOVERNANCE_PLANE;
use systemprompt_manifest::Profile;

use super::CheckResult;

const NAME: &str = "config-planes";
const PLANES: [&str; 3] = [
    GOVERNANCE_PLANE,
    GATEWAY_POLICIES_PLANE,
    ACCESS_CONTROL_PLANE,
];
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn check_config_planes(
    profile: &Profile,
    secrets: &HashMap<String, String>,
) -> CheckResult {
    let mode = profile.unapplied_plane_mode().as_str();
    let Some(url) = secrets
        .get("external_database_url")
        .or_else(|| secrets.get("database_url"))
    else {
        return CheckResult::warn(
            NAME,
            format!("unapplied_planes: {mode}; no database URL, so plane state was not checked"),
        );
    };
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect(url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            return CheckResult::warn(
                NAME,
                format!(
                    "unapplied_planes: {mode}; database unreachable ({error}), plane state not checked"
                ),
            );
        },
    };
    let mut states = Vec::with_capacity(PLANES.len());
    let mut unapplied = Vec::new();
    for plane in PLANES {
        match find_config_plane_applied(&pool, plane).await {
            Ok(true) => states.push(format!("{plane} applied")),
            Ok(false) => {
                states.push(format!("{plane} NOT APPLIED"));
                unapplied.push(plane);
            },
            Err(error) => states.push(format!("{plane} unknown ({error})")),
        }
    }
    pool.close().await;
    let detail = format!("unapplied_planes: {mode}; {}", states.join(", "));
    if unapplied.is_empty() {
        CheckResult::pass(NAME, detail)
    } else {
        CheckResult::warn(NAME, format!("{detail} — {PLANE_NOT_APPLIED_HINT}"))
    }
}

pub(super) fn check_services_source(profile: &Profile) -> CheckResult {
    use systemprompt_loader::bundle::{BundleCache, cache_root};
    const SOURCE: &str = "services-source";
    if profile.services.sources.is_empty() {
        return CheckResult::pass(SOURCE, "no services sources are pinned");
    }
    let cache = BundleCache::new(cache_root(profile));
    if let Some(record) = cache.read_degraded() {
        return CheckResult::warn(
            SOURCE,
            format!(
                "serving the {} services tree since {}: {}",
                record.serving, record.recorded_at, record.error
            ),
        );
    }
    let state = cache.read_state();
    if state.composed_hash.is_empty() {
        return CheckResult::pass(SOURCE, "sources are pinned and not fetched yet");
    }
    if state.last_reconciled_hash.as_deref() != Some(state.composed_hash.as_str()) {
        return CheckResult::warn(
            SOURCE,
            format!(
                "composition {} was fetched but never reconciled; run `systemprompt core \
                 services refresh`",
                state.composed_hash
            ),
        );
    }
    CheckResult::pass(
        SOURCE,
        format!("composition {} is reconciled", state.composed_hash),
    )
}
