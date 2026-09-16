//! Sampling of recent gateway traffic into evaluation cases through the
//! `AiRequestTrace` seam.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{DynAiRequestTrace, TraceSample, TraceSampleFilter};

use crate::error::Result;

#[derive(Clone)]
pub struct SamplerService {
    trace: DynAiRequestTrace,
}

impl std::fmt::Debug for SamplerService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamplerService").finish_non_exhaustive()
    }
}

impl SamplerService {
    #[must_use]
    pub const fn new(trace: DynAiRequestTrace) -> Self {
        Self { trace }
    }

    pub async fn sample(&self, filter: &TraceSampleFilter) -> Result<Vec<TraceSample>> {
        let sampled = self.trace.sample(filter).await?;
        Ok(sampled
            .into_iter()
            .filter(|request| request.response_text.is_some() && !request.messages.is_empty())
            .collect())
    }
}
