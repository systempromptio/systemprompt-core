//! The gateway's inference catalog as the bridge holds it: the last good
//! provider list, the gateway it came from, and how fresh it is.
//!
//! Every model view (the Status card, each host's compatible models, the
//! Desktop subset) is derived from this one value by `model_view`, and only
//! [`InferenceCatalog::apply`] changes it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::types::ProviderHealth;

/// How far the held [`InferenceCatalog`] can be trusted.
///
/// `Stale` keeps the last good providers on screen and says why the latest
/// probe could not refresh them; `since` is when the first failed refresh
/// landed, so a run of failures does not reset the age.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CatalogFreshness {
    #[default]
    Unknown,
    Fresh,
    Stale {
        since: u64,
        error: String,
    },
}

/// The gateway's provider catalog: the one source every model view reads.
///
/// `gateway` is the gateway the providers came from; a probe of another
/// gateway replaces the catalog outright rather than marking it stale.
#[derive(Debug, Clone, Default)]
pub struct InferenceCatalog {
    pub gateway: Option<String>,
    pub providers: Vec<ProviderHealth>,
    pub fetched_at_unix: Option<u64>,
    pub freshness: CatalogFreshness,
}

impl InferenceCatalog {
    /// The reducer rule: a good fetch replaces, a failed one keeps the last
    /// good providers of the same gateway and marks them stale.
    pub fn apply(
        &mut self,
        gateway: &str,
        at_unix: u64,
        fetched: Result<Vec<ProviderHealth>, String>,
    ) {
        let same_gateway = self.gateway.as_deref() == Some(gateway);
        match fetched {
            Ok(providers) => {
                *self = Self {
                    gateway: Some(gateway.to_owned()),
                    providers,
                    fetched_at_unix: Some(at_unix),
                    freshness: CatalogFreshness::Fresh,
                };
            },
            Err(error) if same_gateway && self.fetched_at_unix.is_some() => {
                let since = match &self.freshness {
                    CatalogFreshness::Stale { since, .. } => *since,
                    _ => at_unix,
                };
                self.freshness = CatalogFreshness::Stale { since, error };
            },
            Err(_) => {
                *self = Self {
                    gateway: Some(gateway.to_owned()),
                    ..Self::default()
                };
            },
        }
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        matches!(self.freshness, CatalogFreshness::Stale { .. })
    }
}
