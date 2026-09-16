//! Durable normalized evidence and replacement projections for skill analytics.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod backfill;
mod columns;
mod deltas;
mod projection;
mod queue;
mod repository;
mod service;
mod totals;
mod types;
mod validation;
pub use repository::FeedbackFactsRepository;
pub use service::FactsProcessingService;
pub use types::*;
