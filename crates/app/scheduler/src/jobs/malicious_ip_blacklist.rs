use anyhow::Result;
use async_trait::async_trait;
use systemprompt_core_analytics::detection::{DATACENTER_IP_PREFIXES, HIGH_RISK_COUNTRIES};
use systemprompt_core_database::DbPool;
use systemprompt_core_users::{BanDuration, BanIpParams, BannedIpRepository};
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::{info, warn};

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

        let banned_ip_repo = BannedIpRepository::new(&db_pool)?;

        info!("Starting malicious IP blacklist job");

        let mut banned_count = 0u64;

        let high_volume_ips = find_high_volume_ips(&db_pool).await?;
        banned_count += process_candidates(&high_volume_ips, &banned_ip_repo).await;

        let scanner_ips = find_scanner_ips(&db_pool).await?;
        banned_count += process_candidates(&scanner_ips, &banned_ip_repo).await;

        let datacenter_ips = find_datacenter_ips(&db_pool).await?;
        banned_count += process_candidates(&datacenter_ips, &banned_ip_repo).await;

        let high_risk_country_ips = find_high_risk_country_ips(&db_pool).await?;
        banned_count += process_candidates(&high_risk_country_ips, &banned_ip_repo).await;

        let expired_cleaned = banned_ip_repo.cleanup_expired().await.unwrap_or(0);

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

async fn find_high_volume_ips(pool: &DbPool) -> Result<Vec<MaliciousIpCandidate>> {
    let pg_pool = pool.pool_arc()?;

    let rows = sqlx::query!(
        r#"
        SELECT ip_address, COUNT(*) as session_count
        FROM user_sessions
        WHERE started_at >= NOW() - INTERVAL '24 hours'
          AND ip_address IS NOT NULL
          AND is_bot = false
        GROUP BY ip_address
        HAVING COUNT(*) >= $1
        "#,
        HIGH_REQUEST_THRESHOLD
    )
    .fetch_all(&*pg_pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            r.ip_address.map(|ip| MaliciousIpCandidate {
                ip_address: ip,
                reason: BanReason::HighRequestVolume,
                session_count: r.session_count.unwrap_or(0),
            })
        })
        .collect())
}

async fn find_scanner_ips(pool: &DbPool) -> Result<Vec<MaliciousIpCandidate>> {
    let pg_pool = pool.pool_arc()?;

    let rows = sqlx::query!(
        r#"
        SELECT ip_address, COUNT(*) as session_count
        FROM user_sessions
        WHERE started_at >= NOW() - INTERVAL '24 hours'
          AND ip_address IS NOT NULL
          AND is_scanner = true
        GROUP BY ip_address
        HAVING COUNT(*) >= $1
        "#,
        SCANNER_BAN_THRESHOLD
    )
    .fetch_all(&*pg_pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            r.ip_address.map(|ip| MaliciousIpCandidate {
                ip_address: ip,
                reason: BanReason::ScannerActivity,
                session_count: r.session_count.unwrap_or(0),
            })
        })
        .collect())
}

async fn find_datacenter_ips(pool: &DbPool) -> Result<Vec<MaliciousIpCandidate>> {
    let pg_pool = pool.pool_arc()?;

    let rows = sqlx::query!(
        r#"
        SELECT ip_address, COUNT(*) as session_count
        FROM user_sessions
        WHERE started_at >= NOW() - INTERVAL '24 hours'
          AND ip_address IS NOT NULL
        GROUP BY ip_address
        "#
    )
    .fetch_all(&*pg_pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            r.ip_address.and_then(|ip| {
                if DATACENTER_IP_PREFIXES.iter().any(|p| ip.starts_with(p)) {
                    Some(MaliciousIpCandidate {
                        ip_address: ip,
                        reason: BanReason::DatacenterIp,
                        session_count: r.session_count.unwrap_or(0),
                    })
                } else {
                    None
                }
            })
        })
        .collect())
}

async fn find_high_risk_country_ips(pool: &DbPool) -> Result<Vec<MaliciousIpCandidate>> {
    let pg_pool = pool.pool_arc()?;

    let rows = sqlx::query!(
        r#"
        SELECT ip_address, country, COUNT(*) as session_count
        FROM user_sessions
        WHERE started_at >= NOW() - INTERVAL '24 hours'
          AND ip_address IS NOT NULL
          AND country IS NOT NULL
        GROUP BY ip_address, country
        HAVING COUNT(*) >= $1
        "#,
        HIGH_RISK_COUNTRY_THRESHOLD
    )
    .fetch_all(&*pg_pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let country = r.country.as_deref()?;
            if HIGH_RISK_COUNTRIES.contains(&country) {
                r.ip_address.map(|ip| MaliciousIpCandidate {
                    ip_address: ip,
                    reason: BanReason::HighRiskCountry,
                    session_count: r.session_count.unwrap_or(0),
                })
            } else {
                None
            }
        })
        .collect())
}

async fn process_candidates(candidates: &[MaliciousIpCandidate], repo: &BannedIpRepository) -> u64 {
    let mut banned = 0u64;

    for candidate in candidates {
        if repo.is_banned(&candidate.ip_address).await.unwrap_or(false) {
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

systemprompt_traits::submit_job!(&MaliciousIpBlacklistJob);
