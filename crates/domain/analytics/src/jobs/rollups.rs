//! The analytics rollups the scheduler's `backfill_rollups` job drains: the
//! hours an upgrade queued before `tool_usage_rollup_hourly` and
//! `agent_task_rollup_hourly` existed, rebuilt whole from the ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::projection::ProjectionDrain;
use systemprompt_database::submit_projection_drain;

const BATCH: i32 = 24;
const BUDGET: Duration = Duration::from_secs(15);

submit_projection_drain!(
    "analytics",
    "backfill_rollups",
    ProjectionDrain {
        projection: "tool_usage_rollup_hourly",
        projector: "refresh_tool_usage_rollup_hourly",
        batch: BATCH,
        budget: BUDGET,
    }
);

submit_projection_drain!(
    "analytics",
    "backfill_rollups",
    ProjectionDrain {
        projection: "agent_task_rollup_hourly",
        projector: "refresh_agent_task_rollup_hourly",
        batch: BATCH,
        budget: BUDGET,
    }
);
