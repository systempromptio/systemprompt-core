//! The brain's scheduled jobs.
//!
//! [`BrainReviewJob`] marks facts past their review date and asks someone
//! to review them, and [`BrainRetentionJob`] deletes or retires what has
//! outlived its retention. Both run once per cluster and read the `brain:`
//! profile block and the database pool from the job context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod analyses;
mod retention;
mod review;

pub use analyses::BrainAnalysisJob;
pub use retention::BrainRetentionJob;
pub use review::{BRAIN_REVIEW_STREAM, BrainReviewJob, FACT_REVIEW_DUE, review_due_event};
