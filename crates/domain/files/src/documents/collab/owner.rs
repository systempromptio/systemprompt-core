//! The session-owner lease of a live document, seen from one replica.
//!
//! A replica built with [`OwnerConfig`] claims the owner lease of a document
//! the first time it routes one of its updates and renews it once half the
//! hold has passed. While another replica holds the lease,
//! [`CollabService::update_route`] answers [`UpdateRoute::Forward`] with the
//! holder's advertised address and the caller forwards the raw update there
//! (see the `owner_forward` module), so one replica applies a document's
//! updates and the append never contends. A lapsed lease, a database error
//! or a failed forward all fall back to applying locally: the shared log and
//! its catch-up stay the correctness path, the lease only removes
//! contention. Fan-out is unchanged — the applying replica publishes the
//! update and the event router relays it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use systemprompt_identifiers::DocumentId;

use super::CollabService;
use crate::documents::repository::OwnerClaim;

const DEFAULT_HOLD: Duration = Duration::from_secs(15);
const DEFAULT_FORWARD_TIMEOUT: Duration = Duration::from_secs(3);

/// How this replica takes part in session ownership: the base URL other
/// replicas reach it on (for example `http://10.0.0.7:8080`), how long a
/// claim holds, and how long a forward may take before falling back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerConfig {
    pub address: String,
    pub hold: Duration,
    pub forward_timeout: Duration,
}

impl OwnerConfig {
    #[must_use]
    pub fn at(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            hold: DEFAULT_HOLD,
            forward_timeout: DEFAULT_FORWARD_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateRoute {
    Local,
    Forward { address: String },
}

#[derive(Debug)]
pub(super) struct OwnerRegistry {
    pub(super) id: uuid::Uuid,
    pub(super) config: OwnerConfig,
    pub(super) client: reqwest::Client,
    held: Mutex<HashMap<DocumentId, Instant>>,
}

impl OwnerRegistry {
    fn new(config: OwnerConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.forward_timeout)
            .build()
            .unwrap_or_default();
        Self {
            id: uuid::Uuid::new_v4(),
            config,
            client,
            held: Mutex::new(HashMap::new()),
        }
    }

    fn fresh(&self, document: &DocumentId) -> bool {
        let renew_at = self.config.hold / 2;
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(document)
            .is_some_and(|claimed| claimed.elapsed() < renew_at)
    }

    fn mark(&self, document: &DocumentId, owned: bool) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if owned {
            held.insert(document.clone(), Instant::now());
        } else {
            held.remove(document);
        }
    }
}

impl CollabService {
    #[must_use]
    pub fn with_owner(mut self, config: OwnerConfig) -> Self {
        self.owner = Some(OwnerRegistry::new(config));
        self
    }

    /// Where an update of `document` should be applied: here, or forwarded
    /// to the replica that holds the document's owner lease.
    pub async fn update_route(&self, document: &DocumentId) -> UpdateRoute {
        let Some(owner) = &self.owner else {
            return UpdateRoute::Local;
        };
        if owner.fresh(document) {
            return UpdateRoute::Local;
        }
        let claim = self
            .documents
            .repositories()
            .collab
            .claim_owner(document, owner.id, &owner.config.address, owner.config.hold)
            .await;
        match claim {
            Ok(OwnerClaim::Owned) => {
                owner.mark(document, true);
                UpdateRoute::Local
            },
            Ok(OwnerClaim::Held { address }) if address != owner.config.address => {
                owner.mark(document, false);
                UpdateRoute::Forward { address }
            },
            Ok(OwnerClaim::Held { .. } | OwnerClaim::Lapsed) => UpdateRoute::Local,
            Err(error) => {
                tracing::warn!(
                    document_id = %document,
                    error = %error,
                    "Collab owner lease not claimed; applying the update locally"
                );
                UpdateRoute::Local
            },
        }
    }

    pub(super) async fn release_owner(&self, document: &DocumentId) {
        let Some(owner) = &self.owner else {
            return;
        };
        owner.mark(document, false);
        if let Err(error) = self
            .documents
            .repositories()
            .collab
            .release_owner(document, owner.id)
            .await
        {
            tracing::warn!(
                document_id = %document,
                error = %error,
                "Collab owner lease not released; it lapses when it expires"
            );
        }
    }
}
