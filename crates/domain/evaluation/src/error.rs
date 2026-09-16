//! Typed error boundary for the `systemprompt-evaluation` crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum EvaluationError {
        common: [repository, json],

        #[error("Evaluation state conflict: {0}")]
        ExperimentConflict(String),

        #[error("Evaluation resource not found: {0}")]
        ResourceNotFound(String),

        #[error("Invalid evaluation specification: {0}")]
        InvalidSpec(String),

        #[error("AI request trace unavailable: {0}")]
        Trace(#[from] systemprompt_traits::AiProviderError),

        #[error("Managed revision ownership unavailable: {0}")]
        ManagedRevisions(#[from] systemprompt_traits::ManagedSkillResolverError),

        #[error("Budget exhausted: {required} microdollars required, {available} available")]
        BudgetExhausted { required: i64, available: i64 },
    }
}

impl EvaluationError {
    #[must_use]
    pub const fn budget_exhausted(
        preflight: &crate::experiments::records::ExperimentPreflight,
    ) -> Self {
        Self::BudgetExhausted {
            required: preflight.maximum_cost_microdollars,
            available: preflight.available_microdollars,
        }
    }
}

impl From<sqlx::Error> for EvaluationError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_database::RepositoryError::from(err))
    }
}

pub type Result<T> = std::result::Result<T, EvaluationError>;
