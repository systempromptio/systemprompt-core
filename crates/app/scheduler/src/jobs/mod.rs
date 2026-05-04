//! Built-in job implementations registered via
//! [`systemprompt_provider_contracts::submit_job!`].
//!
//! Each module exposes a single zero-sized type implementing
//! [`systemprompt_traits::Job`]; submission to the inventory registry happens
//! at the bottom of each module.

mod behavioral_analysis;
mod cleanup_empty_contexts;
mod cleanup_inactive_sessions;
mod database_cleanup;
mod ghost_session_cleanup;
mod malicious_ip_blacklist;
mod no_js_cleanup;

pub use behavioral_analysis::BehavioralAnalysisJob;
pub use cleanup_empty_contexts::CleanupEmptyContextsJob;
pub use cleanup_inactive_sessions::CleanupInactiveSessionsJob;
pub use database_cleanup::DatabaseCleanupJob;
pub use ghost_session_cleanup::GhostSessionCleanupJob;
pub use malicious_ip_blacklist::MaliciousIpBlacklistJob;
pub use no_js_cleanup::NoJsCleanupJob;
