//! Resolution and caching of the effective gateway policy.
//!
//! [`PolicyResolver`] loads the global policy rows in ascending
//! `(priority, name)` order and merges them into a single
//! [`GatewayPolicySpec`] — each non-empty section overrides the previous, so
//! the highest-priority row wins. The result is cached for a short TTL.
//!
//! A DB error is a fault governed by [`QuotaFaultMode`]: under `Open` the
//! resolver degrades to a permissive policy, which drops quota windows *and*
//! safety scanning for the request; under `Closed` it returns
//! [`PolicyUnavailable`] and the request is denied. A malformed spec row is
//! always skipped — the remaining rows still merge.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use systemprompt_ai::repository::AiGatewayPolicyRepository;
use systemprompt_models::services::QuotaFaultMode;

pub use systemprompt_ai::{GatewayPolicySpec, QuotaMode, QuotaWindow, SafetyConfig};

const CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
#[error("gateway policy unavailable: {reason}")]
pub struct PolicyUnavailable {
    pub reason: String,
}

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

    pub async fn resolve(
        &self,
        fault_mode: QuotaFaultMode,
    ) -> Result<GatewayPolicySpec, PolicyUnavailable> {
        if let Ok(cache) = self.cache.read()
            && let Some(entry) = cache.as_ref()
            && entry.fetched_at.elapsed() < CACHE_TTL
        {
            return Ok(entry.spec.clone());
        }

        let rows = match self.repo.list_for_global().await {
            Ok(r) => r,
            Err(e) => {
                if fault_mode.is_closed() {
                    tracing::error!(
                        error = %e,
                        fault_mode = fault_mode.as_str(),
                        "Gateway policy read failed; denying the request"
                    );
                    return Err(PolicyUnavailable {
                        reason: e.to_string(),
                    });
                }
                tracing::warn!(
                    error = %e,
                    fault_mode = fault_mode.as_str(),
                    "Gateway policy read failed; falling back to a permissive policy \
                     (quota windows and safety scanning are not applied)"
                );
                return Ok(GatewayPolicySpec::permissive());
            },
        };

        let spec = merge(rows);
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(CachedEntry {
                spec: spec.clone(),
                fetched_at: Instant::now(),
            });
        }
        Ok(spec)
    }
}

fn merge(rows: Vec<systemprompt_ai::GatewayPolicyRow>) -> GatewayPolicySpec {
    let mut merged = GatewayPolicySpec::permissive();
    for row in rows {
        let Ok(spec) = serde_json::from_value::<GatewayPolicySpec>(row.spec) else {
            tracing::warn!(policy_id = %row.id, name = %row.name, "policy spec JSON malformed — skipped");
            continue;
        };
        if !spec.quota_windows.is_empty() || spec.quota_mode.is_warn() {
            merged.quota_mode = spec.quota_mode;
        }
        if !spec.quota_windows.is_empty() {
            merged.quota_windows = spec.quota_windows;
        }
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
