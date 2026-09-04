//! Resolution and caching of the effective gateway policy.
//!
//! [`PolicyResolver`] loads the global policy rows in ascending
//! `(priority, name)` order and merges them into a single
//! [`GatewayPolicySpec`] — each non-empty section overrides the previous, so
//! the highest-priority row wins. The result is cached for a short TTL; a DB
//! error or
//! a malformed spec degrades to a permissive policy rather than failing the
//! request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use systemprompt_ai::repository::AiGatewayPolicyRepository;

pub use systemprompt_ai::{GatewayPolicySpec, QuotaMode, QuotaWindow, SafetyConfig};

const CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct PolicyResolver {
    repo: Arc<AiGatewayPolicyRepository>,
    cache: Arc<RwLock<Option<CachedEntry>>>,
}

impl std::fmt::Debug for PolicyResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyResolver").finish()
    }
}

#[derive(Clone)]
struct CachedEntry {
    spec: GatewayPolicySpec,
    fetched_at: Instant,
}

impl PolicyResolver {
    pub fn from_repository(repo: AiGatewayPolicyRepository) -> Self {
        Self {
            repo: Arc::new(repo),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn resolve(&self) -> GatewayPolicySpec {
        if let Ok(cache) = self.cache.read()
            && let Some(entry) = cache.as_ref()
            && entry.fetched_at.elapsed() < CACHE_TTL
        {
            return entry.spec.clone();
        }

        let rows = match self.repo.list_for_global().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "policy resolve DB error — falling back to permissive");
                return GatewayPolicySpec::permissive();
            },
        };

        let spec = merge(rows);
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(CachedEntry {
                spec: spec.clone(),
                fetched_at: Instant::now(),
            });
        }
        spec
    }
}

fn merge(rows: Vec<systemprompt_ai::GatewayPolicyRow>) -> GatewayPolicySpec {
    let mut merged = GatewayPolicySpec::permissive();
    for row in rows {
        let Ok(spec) = serde_json::from_value::<GatewayPolicySpec>(row.spec) else {
            tracing::warn!(policy_id = %row.id, name = %row.name, "policy spec JSON malformed — skipped");
            continue;
        };
        // Why: same rule as `safety.mode` below. A row that only says
        // `quota_mode: warn` is a real declaration and must survive the merge,
        // or the quota plane keeps refusing after an operator switched it off.
        if !spec.quota_windows.is_empty() || spec.quota_mode.is_warn() {
            merged.quota_mode = spec.quota_mode;
        }
        if !spec.quota_windows.is_empty() {
            merged.quota_windows = spec.quota_windows;
        }
        // Why: `mode` counts as a safety declaration on its own. Without it a
        // policy row that says only `safety: {mode: warn}` would be dropped
        // here and the gateway would keep enforcing, which is the exact
        // failure warn mode exists to avoid.
        if !spec.safety.scanners.is_empty()
            || !spec.safety.block_categories.is_empty()
            || !spec.safety.block_response_categories.is_empty()
            || spec.safety.mode.is_warn()
        {
            merged.safety = spec.safety;
        }
    }
    merged
}
