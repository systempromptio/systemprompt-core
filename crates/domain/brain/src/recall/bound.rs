//! [`BoundMemoryRecall`]: the recall the `brain` member provides, over the
//! brain handle the slot build returns.
//!
//! While the handle is bound it recalls, records and observes through the
//! handle's [`BrainRecallService`](super::BrainRecallService); while it is
//! unbound (the profile disables the brain) a recall answers
//! `skipped=disabled` and a receipt or observation is dropped, exactly as a
//! gateway without a recall engine answers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::brain::{
    MemoryRecall, RecallError, RecallObservation, RecallOutcome, RecallReceipt, RecallRequest,
    SkipReason,
};

use crate::handle::BrainHandleSlot;

#[derive(Debug, Clone)]
pub struct BoundMemoryRecall {
    brain: Arc<BrainHandleSlot>,
}

impl BoundMemoryRecall {
    #[must_use]
    pub const fn new(brain: Arc<BrainHandleSlot>) -> Self {
        Self { brain }
    }
}

#[async_trait::async_trait]
impl MemoryRecall for BoundMemoryRecall {
    fn name(&self) -> &'static str {
        "brain"
    }

    async fn recall(&self, request: &RecallRequest) -> Result<RecallOutcome, RecallError> {
        match self.brain.get() {
            Some(handle) => handle.recall().recall(request).await,
            None => Ok(RecallOutcome::Skipped {
                reason: SkipReason::Disabled,
            }),
        }
    }

    async fn record(&self, receipt: &RecallReceipt) -> Result<(), RecallError> {
        match self.brain.get() {
            Some(handle) => handle.recall().record(receipt).await,
            None => Ok(()),
        }
    }

    async fn observe(&self, observation: &RecallObservation) -> Result<(), RecallError> {
        match self.brain.get() {
            Some(handle) => handle.recall().observe(observation).await,
            None => Ok(()),
        }
    }
}
