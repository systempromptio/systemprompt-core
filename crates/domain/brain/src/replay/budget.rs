//! [`CountingModel`]: wraps the model client a dry run uses, counts the
//! tokens its calls spend and refuses every call once the replay's share
//! of the daily extraction budget is spent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use systemprompt_extension::brain::{BrainModelClient, BrainModelError};
use systemprompt_models::brain::{BrainModelRequest, BrainModelResponse};

pub struct CountingModel {
    inner: Arc<dyn BrainModelClient>,
    budget: u64,
    spent: AtomicU64,
}

impl std::fmt::Debug for CountingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountingModel")
            .field("budget", &self.budget)
            .field("spent", &self.spent())
            .finish_non_exhaustive()
    }
}

impl CountingModel {
    #[must_use]
    pub const fn new(inner: Arc<dyn BrainModelClient>, budget: u64) -> Self {
        Self {
            inner,
            budget,
            spent: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn spent(&self) -> u64 {
        self.spent.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.spent() >= self.budget
    }
}

#[async_trait::async_trait]
impl BrainModelClient for CountingModel {
    async fn complete(
        &self,
        request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        if self.exhausted() {
            return Err(BrainModelError::Refused {
                reason: "the replay token budget is spent".to_owned(),
            });
        }
        let response = self.inner.complete(request).await?;
        let used = u64::from(response.input_tokens) + u64::from(response.output_tokens);
        self.spent.fetch_add(used, Ordering::Relaxed);
        Ok(response)
    }
}
