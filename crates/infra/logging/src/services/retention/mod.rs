//! Log-retention policy and scheduling.
//!
//! [`RetentionPolicy`]/[`RetentionConfig`] define how long log rows are kept;
//! [`RetentionScheduler`] runs the periodic cleanup that enforces them.

pub mod policies;
pub mod scheduler;

pub use policies::{RetentionConfig, RetentionPolicy};
pub use scheduler::RetentionScheduler;
