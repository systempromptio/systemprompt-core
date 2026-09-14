//! Repositories over the `eval_cases` table plus the sampling reader over the
//! `ai_requests` trace owned by `systemprompt-ai`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod experiments;

mod cases;
mod sampling;

pub use cases::EvalCaseRepository;
pub use sampling::SamplingRepository;

use crate::error::Result;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct EvalRepositories {
    pub cases: EvalCaseRepository,
    pub sampling: SamplingRepository,
}

impl EvalRepositories {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            cases: EvalCaseRepository::new(db)?,
            sampling: SamplingRepository::new(db)?,
        })
    }
}
