//! Repositories over the `eval_*` tables plus the sampling reader over the
//! `ai_requests` trace owned by `systemprompt-ai`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cases;
mod judge_calls;
mod results;
mod rubrics;
mod runs;
mod sampling;

pub use cases::EvalCaseRepository;
pub use judge_calls::{EvalJudgeCallRepository, JudgeCallRecord};
pub use results::EvalResultRepository;
pub use rubrics::EvalRubricRepository;
pub use runs::EvalRunRepository;
pub use sampling::SamplingRepository;

use crate::error::Result;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct EvalRepositories {
    pub runs: EvalRunRepository,
    pub cases: EvalCaseRepository,
    pub results: EvalResultRepository,
    pub rubrics: EvalRubricRepository,
    pub sampling: SamplingRepository,
}

impl EvalRepositories {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            runs: EvalRunRepository::new(db)?,
            cases: EvalCaseRepository::new(db)?,
            results: EvalResultRepository::new(db)?,
            rubrics: EvalRubricRepository::new(db)?,
            sampling: SamplingRepository::new(db)?,
        })
    }
}
