use std::sync::Arc;
use systemprompt_evaluation::capabilities::{ExecutionAdmission, paired_variants};
use systemprompt_evaluation::experiments::{ExecutionMode, ExperimentSpec};

#[derive(Debug)]
struct FixtureAdmission;

impl ExecutionAdmission for FixtureAdmission {
    fn admit(&self, spec: &ExperimentSpec) -> systemprompt_evaluation::Result<()> {
        spec.validate()?;
        paired_variants(spec)?;
        if spec.execution_mode != ExecutionMode::Fixture {
            return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
                "The repository harness admits deterministic fixtures only".to_owned(),
            ));
        }
        Ok(())
    }
}

pub fn fixture_admission() -> Arc<dyn ExecutionAdmission> {
    Arc::new(FixtureAdmission)
}
