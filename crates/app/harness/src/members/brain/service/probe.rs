//! [`BrainProbe`]: asks each newly attached instance whether its brain is
//! enabled. It is a [`BackgroundService`], held as `dyn` and run by the
//! composition root, hence `#[async_trait]` on its impl.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use crate::extension::{BackgroundService, CancellationToken};

use super::{BrainService, BrainStatus};
use crate::members::brain::client::{BrainClient, FactQuery};
use crate::members::brain::error::BrainError;

const FIRST_RETRY: Duration = Duration::from_secs(15);
const LAST_RETRY: Duration = Duration::from_secs(300);

/// Reads whether the brain of each instance the service attaches is enabled.
///
/// A one-fact listing that succeeds means enabled, one the instance refuses
/// as disabled (or does not route) means disabled. Any other failure leaves
/// the brain unconfirmed and is asked again after a growing pause, or as soon
/// as another instance attaches.
#[derive(Debug, Clone)]
pub struct BrainProbe {
    service: BrainService,
}

impl BrainProbe {
    #[must_use]
    pub const fn new(service: BrainService) -> Self {
        Self { service }
    }
}

#[async_trait]
impl BackgroundService for BrainProbe {
    fn name(&self) -> &'static str {
        "brain-probe"
    }

    async fn run(&self, cancel: CancellationToken) {
        let mut attachments = self.service.attachments();
        let mut pause = FIRST_RETRY;
        loop {
            attachments.borrow_and_update();
            if let Some((client, epoch)) = self.service.pending_probe() {
                let read = tokio::select! {
                    () = cancel.cancelled() => return,
                    read = availability(&client) => read,
                };
                match read {
                    Ok(status) => {
                        self.service.settle(epoch, status);
                        pause = FIRST_RETRY;
                    },
                    Err(error) => {
                        tracing::warn!(error = %error, retry_secs = pause.as_secs(), "brain availability could not be read");
                        tokio::select! {
                            () = cancel.cancelled() => return,
                            () = tokio::time::sleep(pause) => {},
                            _ = attachments.changed() => {},
                        }
                        pause = pause.saturating_mul(2).min(LAST_RETRY);
                        continue;
                    },
                }
            }
            tokio::select! {
                () = cancel.cancelled() => return,
                changed = attachments.changed() => if changed.is_err() { return },
            }
        }
    }
}

async fn availability(client: &BrainClient) -> Result<BrainStatus, BrainError> {
    let query = FactQuery {
        limit: Some(1),
        ..FactQuery::default()
    };
    match client.facts(&query).await {
        Ok(_) => Ok(BrainStatus::Enabled),
        Err(BrainError::Disabled) => Ok(BrainStatus::Disabled),
        Err(error) => Err(error),
    }
}
