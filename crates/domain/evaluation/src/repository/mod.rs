//! Repositories over the `eval_cases` table. Sampling of the `ai_requests`
//! trace owned by `systemprompt-ai` goes through
//! `systemprompt_traits::AiRequestTrace` (see [`crate::SamplerService`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod experiments;

mod cases;

pub use cases::EvalCaseRepository;

use crate::error::Result;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct EvalRepositories {
    pub cases: EvalCaseRepository,
}

impl EvalRepositories {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            cases: EvalCaseRepository::new(db)?,
        })
    }
}
