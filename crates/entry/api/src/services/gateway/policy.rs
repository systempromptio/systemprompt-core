use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use systemprompt_ai::repository::AiGatewayPolicyRepository;
use systemprompt_database::DbPool;

// The gateway-policy spec types are owned by `systemprompt-ai` so the
// version-controlled `services/gateway/policies.yaml` and the persisted
// `ai_gateway_policies.spec` column share one schema. Re-exported here so
// existing `super::policy::{...}` call sites are unaffected.
pub use systemprompt_ai::{GatewayPolicySpec, QuotaWindow, SafetyConfig};

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
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            repo: Arc::new(
                AiGatewayPolicyRepository::new(db)
                    .map_err(|e| anyhow::anyhow!("policy repo init: {e}"))?,
            ),
            cache: Arc::new(RwLock::new(None)),
        })
    }

    pub async fn resolve(&self) -> GatewayPolicySpec {
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.as_ref() {
                if entry.fetched_at.elapsed() < CACHE_TTL {
                    return entry.spec.clone();
                }
            }
        }

        let rows = match self.repo.find_for_global().await {
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
        if spec.max_input_tokens_per_call.is_some() {
            merged.max_input_tokens_per_call = spec.max_input_tokens_per_call;
        }
        if spec.max_tool_depth.is_some() {
            merged.max_tool_depth = spec.max_tool_depth;
        }
        if !spec.quota_windows.is_empty() {
            merged.quota_windows = spec.quota_windows;
        }
        if !spec.safety.scanners.is_empty() || !spec.safety.block_categories.is_empty() {
            merged.safety = spec.safety;
        }
    }
    merged
}
