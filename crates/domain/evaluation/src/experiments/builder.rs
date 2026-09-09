//! Build an experiment without permitting an unbounded or empty execution
//! matrix.

use super::{ExecutionMode, ExperimentSpec, Objective, VariantSpec};
use crate::Result;
use systemprompt_identifiers::EvalRevisionId;

#[derive(Debug)]
pub struct ExperimentSpecBuilder {
    spec: ExperimentSpec,
}

impl ExperimentSpec {
    pub fn builder(name: impl Into<String>, rubric: EvalRevisionId) -> ExperimentSpecBuilder {
        ExperimentSpecBuilder {
            spec: Self {
                schema_version: 1,
                name: name.into(),
                rubric,
                cases: Vec::new(),
                variants: Vec::new(),
                repetitions: 1,
                budget_microdollars: 0,
                execution_mode: ExecutionMode::Fixture,
                objective: Objective::Quality,
            },
        }
    }
}

impl ExperimentSpecBuilder {
    pub fn cases(mut self, cases: Vec<EvalRevisionId>) -> Self {
        self.spec.cases = cases;
        self
    }

    pub fn variants(mut self, variants: Vec<VariantSpec>) -> Self {
        self.spec.variants = variants;
        self
    }

    pub const fn repetitions(mut self, repetitions: u32) -> Self {
        self.spec.repetitions = repetitions;
        self
    }

    pub const fn budget_microdollars(mut self, budget: i64) -> Self {
        self.spec.budget_microdollars = budget;
        self
    }

    pub const fn execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.spec.execution_mode = mode;
        self
    }

    pub const fn objective(mut self, objective: Objective) -> Self {
        self.spec.objective = objective;
        self
    }

    pub fn build(self) -> Result<ExperimentSpec> {
        self.spec.validate()?;
        Ok(self.spec)
    }
}
