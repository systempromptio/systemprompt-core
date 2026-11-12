//! Per-endpoint circuit breakers for the gateway's upstream calls, built from
//! the endpoint's resolved `resilience:` settings (the endpoint's override,
//! else its provider's), the same settings the internal `ResilientProvider`
//! uses. A breaker is keyed by `provider/endpoint`, so one failing region or
//! wire never trips its siblings. A hop billed to a member is keyed by the
//! endpoint and that member ([`breaker_key`]): one member's exhausted limit or
//! revoked key is that account's failure, not the endpoint's, and never opens
//! the circuit for anyone else.
//!
//! Member-scoped keys make the registry grow with the membership, so past
//! [`REGISTRY_CAP`] entries a closed breaker nobody holds is dropped; a breaker
//! rebuilt later starts closed, which is the state the dropped one was in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_database::resilience::{CircuitBreaker, Probe, ResilienceConfig};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::ResilienceSettings;
use systemprompt_manifest::services::catalog::ResolvedHop;

/// Circuit breakers owned by one gateway, one per provider name, sharing the
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
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn for_endpoint(&self, name: &str, settings: &ResilienceSettings) -> Arc<CircuitBreaker> {
        let mut map = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= REGISTRY_CAP && !map.contains_key(name) {
            map.retain(|_, breaker| Arc::strong_count(breaker) > 1 || breaker.is_open());
        }
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

pub const REGISTRY_CAP: usize = 4096;

#[must_use]
pub fn breaker_key(hop: &ResolvedHop, owner: &UserId) -> String {
    if hop.credential.is_member_paid() {
        format!("{}#{owner}", hop.endpoint_ref())
    } else {
        hop.endpoint_ref().to_string()
    }
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
