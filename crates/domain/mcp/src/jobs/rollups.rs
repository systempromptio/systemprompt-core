//! The tool-usage rollup the scheduler's `backfill_rollups` job drains: the
//! hours an upgrade queued before `tool_usage_rollup_hourly` existed, rebuilt
//! whole from the ledger. The drain binds to the kernel marker
//! [`BackfillRollups`].
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
        projection: "tool_usage_rollup_hourly",
        projector: "refresh_tool_usage_rollup_hourly",
        batch: BATCH,
        budget: BUDGET,
    });
}
