//! The brain's scheduled jobs.
//!
//! [`BrainReviewJob`] marks facts past their review date and asks someone
//! to review them, [`BrainRetentionJob`] deletes or retires what has
//! outlived its retention, [`BrainUsageJob`] writes each fact's usage
//! rates, [`BrainAnalysisJob`] drafts proposals and
//! [`BrainConsolidateJob`] is scaffolding that stays inactive until an
//! extension opts in. All run once per cluster and read the `brain:`
//! profile block and the database pool from the job context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod analyses;
mod consolidate;
mod consolidate_run;
mod retention;
mod review;
mod usage;

pub use analyses::BrainAnalysisJob;
pub use consolidate::{BrainConsolidateJob, ConsolidationInactive, ConsolidationParts};
pub use retention::BrainRetentionJob;
pub use review::{BRAIN_REVIEW_STREAM, BrainReviewJob, FACT_REVIEW_DUE, review_due_event};
pub use usage::{BrainUsageJob, UsageRates, laplace_rates};
