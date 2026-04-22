use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use systemprompt_ai::repository::AiGatewayPolicyRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::TenantId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct QuotaWindow {
    pub window_seconds: i32,
    pub max_requests: Option<i64>,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SafetyConfig {
    #[serde(default)]
    pub scanners: Vec<String>,
    #[serde(default)]
    pub block_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayPolicySpec {
    #[serde(default)]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default)]
    pub max_input_tokens_per_call: Option<u32>,
    #[serde(default)]
    pub max_tool_depth: Option<u32>,
    #[serde(default)]
    pub quota_windows: Vec<QuotaWindow>,
    #[serde(default)]
    pub safety: SafetyConfig,
}

impl GatewayPolicySpec {
    pub fn permissive() -> Self {
        Self::default()
    }

    pub fn model_allowed(&self, model: &str) -> bool {
        self.allowed_models
            .as_deref()
            .is_none_or(|list| list.iter().any(|m| m == model))
    }
}

const CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct PolicyResolver {
    repo: Arc<AiGatewayPolicyRepository>,
    cache: Arc<RwLock<HashMap<String, CachedEntry>>>,
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
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn resolve(&self, tenant_id: Option<&TenantId>) -> GatewayPolicySpec {
        let key = tenant_id
            .map(|t| t.as_str().to_string())
            .unwrap_or_default();

        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(&key) {
                if entry.fetched_at.elapsed() < CACHE_TTL {
                    return entry.spec.clone();
                }
            }
        }

        let rows = match self.repo.find_for_tenant(tenant_id).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "policy resolve DB error — falling back to permissive");
                return GatewayPolicySpec::permissive();
            },
        };

        let spec = merge(rows);
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(
                key,
                CachedEntry {
                    spec: spec.clone(),
                    fetched_at: Instant::now(),
                },
            );
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
        if spec.allowed_models.is_some() {
            merged.allowed_models = spec.allowed_models;
        }
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
