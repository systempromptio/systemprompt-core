//! Sampling of recent gateway traffic into evaluation cases.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use crate::models::{SampleFilter, SampledRequest};
use crate::repository::SamplingRepository;

#[derive(Debug, Clone)]
pub struct SamplerService {
    repository: SamplingRepository,
}

impl SamplerService {
    #[must_use]
    pub const fn new(repository: SamplingRepository) -> Self {
        Self { repository }
    }

    pub async fn sample(&self, filter: &SampleFilter) -> Result<Vec<SampledRequest>> {
        let sampled = self.repository.sample(filter).await?;
        Ok(sampled
            .into_iter()
            .filter(|request| request.response_text.is_some() && !request.messages.is_empty())
            .collect())
    }
}
