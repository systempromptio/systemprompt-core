//! Rechecks retained execution admission before claims and spend reservations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::experiments::{ExperimentSpec, missing};
use systemprompt_identifiers::{EvalExecutionId, UserId};

pub(super) async fn execution(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserId,
    execution: &EvalExecutionId,
    admission: &dyn crate::capabilities::ExecutionAdmission,
) -> crate::Result<()> {
    let spec = sqlx::query_scalar!(
        "SELECT e.spec FROM eval_experiments e JOIN eval_executions x ON x.experiment_id=e.id WHERE e.owner_id=$1 AND x.id=$2",
        owner.as_str(), execution.as_str()
    ).fetch_optional(&mut **tx).await?.ok_or_else(|| missing("Execution unavailable in this scope"))?;
    let spec: ExperimentSpec = serde_json::from_value(spec)?;
    admission.admit(&spec)
}
