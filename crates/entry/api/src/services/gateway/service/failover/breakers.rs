//! Per-provider circuit breakers for the gateway's upstream calls, sharing
//! each provider's `resilience:` settings with the internal
//! `ResilientProvider`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use systemprompt_database::resilience::{CircuitBreaker, Probe, ResilienceConfig};
use systemprompt_models::services::ResilienceSettings;

/// Process-wide circuit breakers, one per provider name, sharing the
/// provider's `resilience:` settings with the internal `ResilientProvider`.
pub struct ProviderBreakers {
    inner: Mutex<HashMap<String, Arc<CircuitBreaker>>>,
}

impl std::fmt::Debug for ProviderBreakers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderBreakers").finish_non_exhaustive()
    }
}

impl ProviderBreakers {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn global() -> &'static Self {
        static BREAKERS: OnceLock<ProviderBreakers> = OnceLock::new();
        BREAKERS.get_or_init(Self::new)
    }

    pub fn for_provider(&self, name: &str, settings: &ResilienceSettings) -> Arc<CircuitBreaker> {
        let mut map = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(map.entry(name.to_owned()).or_insert_with(|| {
            Arc::new(CircuitBreaker::new(
                format!("gateway:{name}"),
                ResilienceConfig::from(settings).breaker,
            ))
        }))
    }
}

impl Default for ProviderBreakers {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn breaker_settings(provider: &str) -> ResilienceSettings {
    systemprompt_loader::ServicesBootstrap::get()
        .ok()
        .and_then(|services| services.ai.providers.get(provider))
        .map(|p| p.resilience)
        .unwrap_or_default()
}

pub(super) fn acquire(breaker: &CircuitBreaker) -> Option<Probe<'_>> {
    breaker.acquire().ok()
}

pub(super) fn settle(probe: Option<Probe<'_>>, healthy: bool) {
    if let Some(probe) = probe {
        if healthy {
            probe.success();
        } else {
            probe.failure();
        }
    }
}
