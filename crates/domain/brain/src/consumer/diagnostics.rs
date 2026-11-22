//! The brain capture consumer's `/health/detail` entry: its state, its
//! pending outbox events, the age of the oldest and when its last round ran.
//! It reads the consumer through the brain handle the slot build returns; until
//! the host starts the consumer, or with the brain disabled, the entry is
//! omitted. It never gates `/readyz`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_extension::{DiagnosticDetail, DiagnosticFailure, DiagnosticProbe};

use crate::handle::BrainHandleSlot;

#[derive(Debug, Clone)]
pub struct BacklogDiagnostic {
    brain: Arc<BrainHandleSlot>,
}

impl BacklogDiagnostic {
    #[must_use]
    pub const fn new(brain: Arc<BrainHandleSlot>) -> Self {
        Self { brain }
    }

    #[must_use]
    pub fn into_probe(self) -> Arc<dyn DiagnosticProbe> {
        Arc::new(self)
    }
}

#[async_trait]
impl DiagnosticProbe for BacklogDiagnostic {
    fn name(&self) -> &'static str {
        "brain"
    }

    fn subject(&self) -> &'static str {
        "backlog"
    }

    async fn probe(&self) -> Result<Option<DiagnosticDetail>, DiagnosticFailure> {
        let Some(consumer) = self.brain.get().and_then(|handle| handle.consumer()) else {
            return Ok(None);
        };
        let snapshot = consumer.backlog();
        let pending = consumer
            .pending()
            .await
            .map_err(|error| DiagnosticFailure {
                reason: "backlog unavailable".into(),
                source: Some(Box::new(error)),
            })?;
        let mut detail = DiagnosticDetail::new();
        detail.insert("state".to_owned(), json!(snapshot.state));
        detail.insert("pending".to_owned(), json!(pending.pending));
        detail.insert("oldest_seconds".to_owned(), json!(pending.oldest_seconds));
        detail.insert("last_round".to_owned(), json!(snapshot.last_round));
        Ok(Some(detail))
    }
}
