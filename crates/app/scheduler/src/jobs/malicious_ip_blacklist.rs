use anyhow::Result;
use async_trait::async_trait;
use systemprompt_analytics::detection::{DATACENTER_IP_PREFIXES, HIGH_RISK_COUNTRIES};
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use systemprompt_users::{BanDuration, BanIpParams, BannedIpRepository};
use tracing::{info, warn};

use crate::repository::{IpSessionRecord, SecurityRepository};

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

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        let security_repo = SecurityRepository::new(&db_pool)?;
        let banned_ip_repo = BannedIpRepository::new(&db_pool)?;

        info!("Starting malicious IP blacklist job");

        let mut banned_count = 0u64;

        let high_volume_candidates = find_high_volume_candidates(&security_repo).await?;
        banned_count += process_candidates(&high_volume_candidates, &banned_ip_repo).await;

        let scanner_candidates = find_scanner_candidates(&security_repo).await?;
        banned_count += process_candidates(&scanner_candidates, &banned_ip_repo).await;

        let datacenter_candidates = find_datacenter_candidates(&security_repo).await?;
        banned_count += process_candidates(&datacenter_candidates, &banned_ip_repo).await;

        let high_risk_candidates = find_high_risk_country_candidates(&security_repo).await?;
        banned_count += process_candidates(&high_risk_candidates, &banned_ip_repo).await;

        let expired_cleaned = match banned_ip_repo.cleanup_expired().await {
            Ok(count) => count,
            Err(e) => {
                warn!(error = %e, "Failed to cleanup expired bans");
                0
            }
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

async fn find_high_volume_candidates(repo: &SecurityRepository) -> Result<Vec<MaliciousIpCandidate>> {
    let records = repo.find_high_volume_ips(HIGH_REQUEST_THRESHOLD).await?;
    Ok(records_to_candidates(records, BanReason::HighRequestVolume))
}

async fn find_scanner_candidates(repo: &SecurityRepository) -> Result<Vec<MaliciousIpCandidate>> {
    let records = repo.find_scanner_ips(SCANNER_BAN_THRESHOLD).await?;
    Ok(records_to_candidates(records, BanReason::ScannerActivity))
}

async fn find_datacenter_candidates(repo: &SecurityRepository) -> Result<Vec<MaliciousIpCandidate>> {
    let records = repo.find_recent_ips().await?;
    Ok(records
        .into_iter()
        .filter_map(|r| {
            let ip = r.ip_address?;
            if DATACENTER_IP_PREFIXES.iter().any(|p| ip.starts_with(p)) {
                Some(MaliciousIpCandidate {
                    ip_address: ip,
                    reason: BanReason::DatacenterIp,
                    session_count: r.session_count,
                })
            } else {
                None
            }
        })
        .collect())
}

async fn find_high_risk_country_candidates(
    repo: &SecurityRepository,
) -> Result<Vec<MaliciousIpCandidate>> {
    let records = repo
        .find_high_risk_country_ips(HIGH_RISK_COUNTRY_THRESHOLD)
        .await?;
    Ok(records
        .into_iter()
        .filter_map(|r| {
            let country = r.country.as_deref()?;
            if HIGH_RISK_COUNTRIES.contains(&country) {
                r.ip_address.map(|ip| MaliciousIpCandidate {
                    ip_address: ip,
                    reason: BanReason::HighRiskCountry,
                    session_count: r.session_count,
                })
            } else {
                None
            }
        })
        .collect())
}

fn records_to_candidates(records: Vec<IpSessionRecord>, reason: BanReason) -> Vec<MaliciousIpCandidate> {
    records
        .into_iter()
        .filter_map(|r| {
            r.ip_address.map(|ip| MaliciousIpCandidate {
                ip_address: ip,
                reason,
                session_count: r.session_count,
            })
        })
        .collect()
}

async fn process_candidates(candidates: &[MaliciousIpCandidate], repo: &BannedIpRepository) -> u64 {
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
            }
        };

        if is_already_banned {
            continue;
        }

        let params = BanIpParams::new(
            &candidate.ip_address,
            candidate.reason.as_str(),
            BanDuration::Days(BAN_DURATION_DAYS),
            "malicious_ip_blacklist",
        );

        match repo.ban_ip(params).await {
            Ok(()) => {
                warn!(
                    ip = %candidate.ip_address,
                    reason = ?candidate.reason,
                    sessions = candidate.session_count,
                    "Banned malicious IP"
                );
                banned += 1;
            }
            Err(e) => {
                warn!(
                    ip = %candidate.ip_address,
                    error = %e,
                    "Failed to ban IP"
                );
            }
        }
    }

    banned
}

systemprompt_provider_contracts::submit_job!(&MaliciousIpBlacklistJob);
