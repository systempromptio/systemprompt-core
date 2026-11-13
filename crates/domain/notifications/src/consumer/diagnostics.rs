//! The notification consumer's `/health/detail` entry: its state and the
//! delivery backlog. The extension provides an empty [`LateBound`] consumer
//! slot that the host binds once it has started the consumer; the probe reads
//! the backlog through it. It never gates `/readyz`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_extension::{DiagnosticDetail, DiagnosticFailure, DiagnosticProbe, LateBound};

use super::NotificationsConsumer;

pub type NotificationsConsumerSlot = LateBound<NotificationsConsumer>;

#[derive(Debug, Clone)]
pub struct BacklogDiagnostic {
    consumer: NotificationsConsumerSlot,
}

impl BacklogDiagnostic {
    #[must_use]
    pub const fn new(consumer: NotificationsConsumerSlot) -> Self {
        Self { consumer }
    }

    #[must_use]
    pub fn into_probe(self) -> Arc<dyn DiagnosticProbe> {
        Arc::new(self)
    }
}

#[async_trait]
impl DiagnosticProbe for BacklogDiagnostic {
    fn name(&self) -> &'static str {
        "notifications"
    }

    fn subject(&self) -> &'static str {
        "backlog"
    }

    async fn probe(&self) -> Result<Option<DiagnosticDetail>, DiagnosticFailure> {
        let unavailable = |source| DiagnosticFailure {
            reason: "backlog unavailable".into(),
            source,
        };
        let consumer = self.consumer.get().ok_or_else(|| unavailable(None))?;
        let backlog = consumer
            .backlog()
            .await
            .map_err(|error| unavailable(Some(Box::new(error))))?;
        let mut detail = DiagnosticDetail::new();
        detail.insert("state".to_owned(), json!(backlog.state));
        detail.insert("pending".to_owned(), json!(backlog.pending));
        detail.insert("oldest_seconds".to_owned(), json!(backlog.oldest_seconds));
        Ok(Some(detail))
    }
}
