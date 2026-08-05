//! Typed error boundary for the `systemprompt-evaluation` crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum EvaluationError {
        common: [repository, json],

        #[error("AI provider request failed: {0}")]
        Ai(String),

        #[error("Run not found: {0}")]
        RunNotFound(String),

        #[error("Rubric not found: {0}")]
        RubricNotFound(String),

        #[error("Judge verdict unparseable: {0}")]
        JudgeParse(String),

        #[error("Replay source incomplete: {0}")]
        ReplaySource(String),

        #[error("Budget exhausted: spent {spent} of {budget} microdollars")]
        BudgetExhausted { spent: i64, budget: i64 },
    }
}

impl From<sqlx::Error> for EvaluationError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_database::RepositoryError::from(err))
    }
}

pub type Result<T> = std::result::Result<T, EvaluationError>;
