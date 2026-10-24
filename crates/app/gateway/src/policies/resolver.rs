//! Resolution and caching of the effective gateway policy.
//!
//! [`PolicyResolver`] loads the global policy rows in ascending
//! `(priority, name)` order and merges them into a single
//! [`GatewayPolicySpec`] — each non-empty section overrides the previous, so
//! the highest-priority row wins. The result is cached for a short TTL, and
//! dropped as soon as a policy write on any replica advances the
//! [`CacheDomain::GatewayPolicy`] epoch. While no invalidation relay is
//! listening the TTL drops to a few seconds, since a write elsewhere cannot
//! reach this cache.
//!
//! An empty policy table on a database where the `gateway_policies` plane was
//! never applied follows the profile's `governance.unapplied_planes`: `deny`
//! refuses with [`PolicyUnavailable::NotApplied`] whatever the fault mode,
//! `warn` (the default) serves the permissive spec and warns on every request.
//! An applied plane that declares no policy is the permissive spec.
//!
//! A DB error is a fault governed by [`QuotaFaultMode`]: under `Open` the
//! resolver degrades to a permissive policy, which drops quota windows *and*
//! safety scanning for the request; under `Closed` it returns
//! [`PolicyUnavailable`] and the request is denied. A malformed spec row is a
//! fault in either mode: the policy it would have contributed is unknown, so
//! the resolver returns [`PolicyUnavailable`] rather than merging the rest from
//! a permissive base. A row that decodes but declares a quota window with no
//! span (or two) is malformed in the same way: it never becomes a one-second
//! bucket.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use systemprompt_ai::repository::AiGatewayPolicyRepository;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::AiGatewayPolicyId;
use systemprompt_manifest::profile::UnappliedPlaneMode;
use systemprompt_manifest::services::QuotaFaultMode;
use systemprompt_traits::{CacheDomain, CacheEpochs};

use super::GatewayPolicySpec;
use super::plane::GATEWAY_POLICIES_PLANE;

const CACHE_TTL: Duration = Duration::from_secs(60);
const UNRELAYED_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum PolicyUnavailable {
    #[error("gateway policy unavailable: the policy store could not be read")]
    Read(#[source] systemprompt_traits::BoxedSource),
    #[error(
        "gateway policy unavailable: the gateway_policies plane is not applied — run \
         `systemprompt admin config apply`"
    )]
    NotApplied,
    #[error("gateway policy unavailable: policy '{name}' is malformed")]
    Malformed {
        name: String,
        #[source]
        source: MalformedPolicy,
    },
}

#[derive(Clone)]
pub struct PolicyResolver {
    repo: Arc<AiGatewayPolicyRepository>,
    cache: Arc<RwLock<Option<CachedEntry>>>,
    epochs: CacheEpochs,
    unapplied: UnappliedPlaneMode,
}

impl std::fmt::Debug for PolicyResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyResolver").finish()
    }
}

#[derive(Clone)]
struct CachedEntry {
    spec: GatewayPolicySpec,
    unapplied: bool,
    fetched_at: Instant,
    epoch: u64,
}

impl PolicyResolver {
    pub fn from_repository(repo: AiGatewayPolicyRepository, epochs: CacheEpochs) -> Self {
        let unapplied = ProfileBootstrap::get().map_or_else(
            |_| UnappliedPlaneMode::default(),
            systemprompt_manifest::Profile::unapplied_plane_mode,
        );
        Self {
            repo: Arc::new(repo),
            cache: Arc::new(RwLock::new(None)),
            epochs,
            unapplied,
        }
    }

    #[must_use]
    pub const fn with_unapplied_mode(mut self, mode: UnappliedPlaneMode) -> Self {
        self.unapplied = mode;
        self
    }

    pub async fn resolve(
        &self,
        fault_mode: QuotaFaultMode,
    ) -> Result<GatewayPolicySpec, PolicyUnavailable> {
        let epoch = self.epochs.current(CacheDomain::GatewayPolicy);
        let ttl = if self.epochs.relay_live() {
            CACHE_TTL
        } else {
            UNRELAYED_CACHE_TTL
        };
        if let Ok(cache) = self.cache.read()
            && let Some(entry) = cache.as_ref()
            && entry.fetched_at.elapsed() < ttl
            && entry.epoch == epoch
        {
            if entry.unapplied {
                warn_unapplied();
            }
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
                    return Err(PolicyUnavailable::Read(e.into()));
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

        let unapplied = rows.is_empty() && !self.plane_applied().await?;
        let spec = merge_policy_rows(rows).map_err(|malformed| {
            tracing::error!(
                policy_id = %malformed.policy_id,
                name = %malformed.name,
                error = %malformed.error,
                "Gateway policy row is malformed; denying the request"
            );
            PolicyUnavailable::Malformed {
                name: malformed.name.clone(),
                source: malformed,
            }
        })?;
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(CachedEntry {
                spec: spec.clone(),
                unapplied,
                fetched_at: Instant::now(),
                epoch,
            });
        }
        Ok(spec)
    }

    async fn plane_applied(&self) -> Result<bool, PolicyUnavailable> {
        let applied = match self.repo.find_plane_applied(GATEWAY_POLICIES_PLANE).await {
            Ok(applied) => applied,
            Err(error) => {
                tracing::error!(%error, "gateway_policies apply state unreadable; denying the request");
                return Err(PolicyUnavailable::Read(error.into()));
            },
        };
        if applied {
            return Ok(true);
        }
        if self.unapplied.denies() {
            tracing::error!(
                "gateway_policies plane not applied (unapplied_planes: deny); denying the \
                 request until `systemprompt admin config apply` writes it"
            );
            return Err(PolicyUnavailable::NotApplied);
        }
        warn_unapplied();
        Ok(false)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("gateway policy '{name}' ({policy_id}) is malformed: {error}")]
pub struct MalformedPolicy {
    pub policy_id: AiGatewayPolicyId,
    pub name: String,
    pub error: MalformedSpec,
}

#[derive(Debug, thiserror::Error)]
pub enum MalformedSpec {
    #[error(transparent)]
    Decode(#[from] serde_json::Error),
    #[error("quota_windows[{index}]: {reason}")]
    QuotaWindow { index: usize, reason: &'static str },
}

// JSON: a stored policy row carries its spec as an untyped jsonb column
fn decode_spec(value: serde_json::Value) -> Result<GatewayPolicySpec, MalformedSpec> {
    let spec = serde_json::from_value::<GatewayPolicySpec>(value)?;
    for (index, window) in spec.quota_windows.iter().enumerate() {
        window
            .validate_span()
            .map_err(|reason| MalformedSpec::QuotaWindow { index, reason })?;
    }
    Ok(spec)
}

pub fn merge_policy_rows(
    rows: Vec<systemprompt_ai::GatewayPolicyRow>,
) -> Result<GatewayPolicySpec, MalformedPolicy> {
    let mut merged = GatewayPolicySpec::permissive();
    for row in rows {
        let spec = decode_spec(row.spec).map_err(|error| MalformedPolicy {
            policy_id: row.id,
            name: row.name,
            error,
        })?;
        if !spec.quota_windows.is_empty() || spec.quota_mode.is_warn() {
            merged.quota_mode = spec.quota_mode;
        }
        if !spec.quota_windows.is_empty() {
            merged.quota_windows = spec.quota_windows;
        }
        if !spec.rate_windows.is_empty() {
            merged.rate_windows = spec.rate_windows;
        }
        if !spec.safety.scanners.is_empty()
            || !spec.safety.block_categories.is_empty()
            || !spec.safety.block_response_categories.is_empty()
            || spec.safety.mode.is_warn()
        {
            merged.safety = spec.safety;
        }
    }
    Ok(merged)
}

fn warn_unapplied() {
    tracing::warn!(
        "gateway_policies plane not applied (unapplied_planes: warn); serving this request \
         with no quota or safety policy until `systemprompt admin config apply` writes it"
    );
}
