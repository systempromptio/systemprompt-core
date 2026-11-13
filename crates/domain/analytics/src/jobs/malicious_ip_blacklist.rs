//! Periodic job that classifies malicious IPs based on request patterns,
//! scanner activity, datacenter ranges, and high-risk country fan-out.
//! Candidates are banned only when the job's `enforce` config flag is set;
//! otherwise they are logged for review.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;

use systemprompt_traits::{
    DynIpBanStore, IpBanRequest, IpBanStore, Job, JobContext, JobResult, ProviderError,
    ProviderResult, SessionStore,
};
use systemprompt_traits::session_store::{IpSessionCount, IpSessionQuery};
use tracing::{info, warn};

use crate::Result;
use crate::detection::{DATACENTER_IP_PREFIXES, HIGH_RISK_COUNTRIES};
use crate::repository::AnalyticsRepositories;

const HIGH_REQUEST_THRESHOLD: i64 = 100;
const SCANNER_BAN_THRESHOLD: i64 = 3;
const HIGH_RISK_COUNTRY_THRESHOLD: i64 = 5;
const BAN_DURATION_DAYS: i64 = 14;

#[derive(Debug, Clone, Copy)]
pub struct MaliciousIpBlacklistJob;

struct MaliciousIpCandidate {
    ip_address: String,
    reason: BanReason,
    session_count: i64,
}

#[derive(Debug, Clone, Copy)]
enum BanReason {
    HighRequestVolume,
    ScannerActivity,
    DatacenterIp,
    HighRiskCountry,
}

impl BanReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::HighRequestVolume => "Automated: high request volume",
            Self::ScannerActivity => "Automated: scanner activity detected",
            Self::DatacenterIp => "Automated: known datacenter IP range",
            Self::HighRiskCountry => "Automated: high-risk country pattern",
        }
    }
}

#[async_trait]
impl Job for MaliciousIpBlacklistJob {
    fn name(&self) -> &'static str {
        "malicious_ip_blacklist"
    }

    fn description(&self) -> &'static str {
        "Detects and blacklists malicious IPs based on request patterns"
    }

    fn schedule(&self) -> &'static str {
        "0 0 */6 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let sessions = Arc::clone(&ctx.get::<Arc<AnalyticsRepositories>>()?.session_store);
        let banned_ip_repo = Arc::clone(ctx.get::<DynIpBanStore>()?);

        info!("Starting malicious IP blacklist job");

        let enforce = ctx.enforce();
        let mut banned_count = 0u64;

        let high_volume_candidates = find_high_volume_candidates(sessions.as_ref())
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        banned_count +=
            process_candidates(&high_volume_candidates, banned_ip_repo.as_ref(), enforce).await;

        let scanner_candidates = find_scanner_candidates(sessions.as_ref())
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        banned_count +=
            process_candidates(&scanner_candidates, banned_ip_repo.as_ref(), enforce).await;

        let datacenter_candidates = find_datacenter_candidates(sessions.as_ref())
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        banned_count +=
            process_candidates(&datacenter_candidates, banned_ip_repo.as_ref(), enforce).await;

        let high_risk_candidates = find_high_risk_country_candidates(sessions.as_ref())
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        banned_count +=
            process_candidates(&high_risk_candidates, banned_ip_repo.as_ref(), enforce).await;

        let expired_cleaned = match banned_ip_repo.cleanup_expired().await {
            Ok(count) => count,
            Err(e) => {
                warn!(error = %e, "Failed to cleanup expired bans");
                0
            },
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            banned = banned_count,
            expired_cleaned = expired_cleaned,
            duration_ms = duration_ms,
            "Malicious IP blacklist job completed"
        );

        Ok(JobResult::success()
            .with_stats(banned_count, 0)
            .with_duration(duration_ms))
    }
}

async fn find_high_volume_candidates(
    sessions: &dyn SessionStore,
) -> Result<Vec<MaliciousIpCandidate>> {
    let records = sessions
        .ip_session_counts(IpSessionQuery::HumanAtLeast(HIGH_REQUEST_THRESHOLD))
        .await?;
    Ok(records_to_candidates(records, BanReason::HighRequestVolume))
}

async fn find_scanner_candidates(sessions: &dyn SessionStore) -> Result<Vec<MaliciousIpCandidate>> {
    let records = sessions
        .ip_session_counts(IpSessionQuery::ScannerAtLeast(SCANNER_BAN_THRESHOLD))
        .await?;
    Ok(records_to_candidates(records, BanReason::ScannerActivity))
}

async fn find_datacenter_candidates(
    sessions: &dyn SessionStore,
) -> Result<Vec<MaliciousIpCandidate>> {
    let records = sessions.ip_session_counts(IpSessionQuery::All).await?;
    Ok(records
        .into_iter()
        .filter(|r| {
            DATACENTER_IP_PREFIXES
                .iter()
                .any(|p| r.ip_address.starts_with(p))
        })
        .map(|r| MaliciousIpCandidate {
            ip_address: r.ip_address,
            reason: BanReason::DatacenterIp,
            session_count: r.session_count,
        })
        .collect())
}

async fn find_high_risk_country_candidates(
    sessions: &dyn SessionStore,
) -> Result<Vec<MaliciousIpCandidate>> {
    let records = sessions
        .ip_session_counts(IpSessionQuery::CountryAtLeast(HIGH_RISK_COUNTRY_THRESHOLD))
        .await?;
    Ok(records
        .into_iter()
        .filter(|r| {
            r.country
                .as_deref()
                .is_some_and(|country| HIGH_RISK_COUNTRIES.contains(&country))
        })
        .map(|r| MaliciousIpCandidate {
            ip_address: r.ip_address,
            reason: BanReason::HighRiskCountry,
            session_count: r.session_count,
        })
        .collect())
}

fn records_to_candidates(
    records: Vec<IpSessionCount>,
    reason: BanReason,
) -> Vec<MaliciousIpCandidate> {
    records
        .into_iter()
        .map(|r| MaliciousIpCandidate {
            ip_address: r.ip_address,
            reason,
            session_count: r.session_count,
        })
        .collect()
}

async fn process_candidates(
    candidates: &[MaliciousIpCandidate],
    repo: &dyn IpBanStore,
    enforce: bool,
) -> u64 {
    let mut banned = 0u64;

    for candidate in candidates {
        let is_already_banned = match repo.is_banned(&candidate.ip_address).await {
            Ok(banned) => banned,
            Err(e) => {
                warn!(
                    ip = %candidate.ip_address,
                    error = %e,
                    "Failed to check ban status"
                );
                continue;
            },
        };

        if is_already_banned {
            continue;
        }

        if !enforce {
            info!(
                ip = %candidate.ip_address,
                reason = ?candidate.reason,
                sessions = candidate.session_count,
                "enforce disabled: IP qualifies for a ban but was not banned"
            );
            continue;
        }

        let request = IpBanRequest {
            ip_address: &candidate.ip_address,
            reason: candidate.reason.as_str(),
            duration_days: BAN_DURATION_DAYS,
            source_fingerprint: None,
            ban_source: "malicious_ip_blacklist",
        };

        match repo.ban_ip(request).await {
            Ok(()) => {
                warn!(
                    ip = %candidate.ip_address,
                    reason = ?candidate.reason,
                    sessions = candidate.session_count,
                    "Banned malicious IP"
                );
                banned += 1;
            },
            Err(e) => {
                warn!(
                    ip = %candidate.ip_address,
                    error = %e,
                    "Failed to ban IP"
                );
            },
        }
    }

    banned
}

systemprompt_traits::submit_job!("analytics", &MaliciousIpBlacklistJob);
