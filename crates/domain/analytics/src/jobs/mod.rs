//! Scheduled jobs and retention contributions of the analytics domain.
//!
//! The session-classification jobs (ghost and no-JS bot marking, the on-demand
//! geo backfill), the two detectors that feed the IP ban list (behavioural
//! analysis, the malicious-IP blacklist) and the link-click projection
//! drainer are contributed by
//! [`AnalyticsExtension`](crate::AnalyticsExtension). The detectors write bans
//! through the `DynIpBanStore` their job context carries and fail when it is
//! absent. The scheduler's kernel jobs reach this crate's tables through the
//! retention purges contributed with the jobs and the usage-rollup drains
//! bound to the kernel's `BackfillRollups` marker.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod backfill_session_geo;
mod behavioral_analysis;
mod ghost_session_cleanup;
mod link_facts_rollup;
mod malicious_ip_blacklist;
mod no_js_cleanup;
mod retention;

pub use backfill_session_geo::BackfillSessionGeoJob;
pub use behavioral_analysis::BehavioralAnalysisJob;
pub use ghost_session_cleanup::GhostSessionCleanupJob;
pub use link_facts_rollup::LinkFactsRollupJob;
pub use malicious_ip_blacklist::MaliciousIpBlacklistJob;
pub use no_js_cleanup::NoJsCleanupJob;
pub use retention::{EngagementEventsPurge, FingerprintReputationPurge};
