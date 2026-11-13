//! Scheduled jobs and retention contributions of the analytics domain.
//!
//! The session-classification jobs (ghost and no-JS bot marking, the on-demand
//! geo backfill) and the two detectors that feed the IP ban list (behavioural
//! analysis, the malicious-IP blacklist) are submitted with `submit_job!` and
//! returned from the extension's `jobs()`. The detectors write bans through
//! the `DynIpBanStore` their job context carries and fail when it is absent.
//! The scheduler's kernel jobs reach this crate's tables through the
//! retention purges and the usage rollups registered here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod backfill_session_geo;
mod behavioral_analysis;
mod ghost_session_cleanup;
mod malicious_ip_blacklist;
mod no_js_cleanup;
mod retention;
mod rollups;

pub use backfill_session_geo::BackfillSessionGeoJob;
pub use behavioral_analysis::BehavioralAnalysisJob;
pub use ghost_session_cleanup::GhostSessionCleanupJob;
pub use malicious_ip_blacklist::MaliciousIpBlacklistJob;
pub use no_js_cleanup::NoJsCleanupJob;
