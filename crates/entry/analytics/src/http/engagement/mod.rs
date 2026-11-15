//! `/track/engagement`: single and batch engagement records, with conversion
//! marking on the caller's session.
//!
//! The routes are declared relative to the `/track/engagement` base (`/` and
//! `/batch`) and cannot answer outside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handlers;

pub use handlers::{BatchResponse, EngagementBatchInput};
pub(crate) use handlers::{record_engagement, record_engagement_batch};
