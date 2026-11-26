//! The AI rollups the scheduler's `backfill_rollups` job drains: the hours an
//! upgrade queued before `ai_usage_rollup_hourly`, `ai_context_rollup_hourly`
//! and `ai_gateway_session_rollup_hourly` existed, rebuilt whole from the
//! ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_extension::projection::kernel::BackfillRollups;
use systemprompt_extension::{Contributions, ProjectionDrain};

const BATCH: i32 = 24;
const BUDGET: Duration = Duration::from_secs(15);

pub(super) fn contribute(c: &mut Contributions<'_>) {
    c.projection::<BackfillRollups>(ProjectionDrain {
        projection: "ai_usage_rollup_hourly",
        projector: "refresh_ai_usage_rollup_hourly",
        batch: BATCH,
        budget: BUDGET,
    })
    .projection::<BackfillRollups>(ProjectionDrain {
        projection: "ai_context_rollup_hourly",
        projector: "refresh_ai_context_rollup_hourly",
        batch: BATCH,
        budget: BUDGET,
    })
    .projection::<BackfillRollups>(ProjectionDrain {
        projection: "ai_gateway_session_rollup_hourly",
        projector: "refresh_ai_gateway_session_rollup_hourly",
        batch: BATCH,
        budget: BUDGET,
    });
}
