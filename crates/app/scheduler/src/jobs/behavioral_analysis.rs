use anyhow::Result;
use async_trait::async_trait;
use systemprompt_core_analytics::{
    FingerprintRepository, FingerprintReputation, FlagReason, ABUSE_THRESHOLD_FOR_BAN,
    HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM, SUSTAINED_VELOCITY_MINUTES,
};
use systemprompt_core_database::DbPool;
use systemprompt_core_users::{BanDuration, BanIpParams, BannedIpRepository};
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::{info, warn};

const SESSION_ABUSE_THRESHOLD: i32 = 10;

#[derive(Debug, Clone, Copy)]
pub struct BehavioralAnalysisJob;

struct AnalysisResult {
    flag_reasons: Vec<FlagReason>,
    new_score: i32,
}

struct AnalysisStats {
    flagged: u64,
    banned: u64,
}

#[async_trait]
impl Job for BehavioralAnalysisJob {
    fn name(&self) -> &'static str {
        "behavioral_analysis"
    }

    fn description(&self) -> &'static str {
        "Analyzes fingerprint behavior patterns and flags suspicious activity"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        let fingerprint_repo = FingerprintRepository::new(&db_pool)?;
        let banned_ip_repo = BannedIpRepository::new(&db_pool)?;

        info!("Starting behavioral analysis job");

        let fingerprints = fingerprint_repo.get_fingerprints_for_analysis().await?;
        let stats = process_fingerprints(&fingerprints, &fingerprint_repo, &banned_ip_repo).await;
        let expired_cleaned = banned_ip_repo.cleanup_expired().await.unwrap_or(0);

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            analyzed = fingerprints.len(),
            flagged = stats.flagged,
            banned = stats.banned,
            expired_cleaned = expired_cleaned,
            duration_ms = duration_ms,
            "Behavioral analysis completed"
        );

        Ok(JobResult::success()
            .with_stats(stats.flagged + stats.banned, 0)
            .with_duration(duration_ms))
    }
}

async fn process_fingerprints(
    fingerprints: &[FingerprintReputation],
    fingerprint_repo: &FingerprintRepository,
    banned_ip_repo: &BannedIpRepository,
) -> AnalysisStats {
    let mut stats = AnalysisStats {
        flagged: 0,
        banned: 0,
    };

    for fp in fingerprints {
        let analysis = analyze_fingerprint(fp);

        if flag_fingerprint_if_needed(fp, &analysis, fingerprint_repo).await {
            stats.flagged += 1;
        }

        if ban_ip_if_needed(fp, banned_ip_repo).await {
            stats.banned += 1;
        }
    }

    stats
}

fn analyze_fingerprint(fp: &FingerprintReputation) -> AnalysisResult {
    let mut flag_reasons = Vec::new();
    let mut reputation_delta = 0i32;

    if fp.total_request_count > HIGH_REQUEST_THRESHOLD {
        flag_reasons.push(FlagReason::HighRequestCount);
        reputation_delta -= 10;
    }

    if fp.peak_requests_per_minute > HIGH_VELOCITY_RPM
        && fp.sustained_high_velocity_minutes >= SUSTAINED_VELOCITY_MINUTES
    {
        flag_reasons.push(FlagReason::SustainedVelocity);
        reputation_delta -= 15;
    }

    if fp.total_session_count > SESSION_ABUSE_THRESHOLD {
        flag_reasons.push(FlagReason::ExcessiveSessions);
        reputation_delta -= 20;
    }

    if fp.reputation_score < 20 && !fp.is_flagged {
        flag_reasons.push(FlagReason::ReputationDecay);
    }

    let new_score = (fp.reputation_score + reputation_delta).clamp(0, 100);

    AnalysisResult {
        flag_reasons,
        new_score,
    }
}

async fn flag_fingerprint_if_needed(
    fp: &FingerprintReputation,
    analysis: &AnalysisResult,
    repo: &FingerprintRepository,
) -> bool {
    let Some(primary_reason) = analysis.flag_reasons.first() else {
        return false;
    };

    let result = repo
        .flag_fingerprint(&fp.fingerprint_hash, *primary_reason, analysis.new_score)
        .await;

    log_flag_result(
        &fp.fingerprint_hash,
        &analysis.flag_reasons,
        analysis.new_score,
        &result,
    )
}

#[allow(clippy::cognitive_complexity)]
fn log_flag_result(
    fingerprint: &str,
    reasons: &[FlagReason],
    new_score: i32,
    result: &Result<(), anyhow::Error>,
) -> bool {
    match result {
        Ok(()) => {
            warn!(fingerprint = %fingerprint, reasons = ?reasons, new_score = new_score, "Flagged fingerprint");
            true
        },
        Err(e) => {
            warn!(fingerprint = %fingerprint, error = %e, "Failed to flag fingerprint");
            false
        },
    }
}

async fn ban_ip_if_needed(fp: &FingerprintReputation, repo: &BannedIpRepository) -> bool {
    if fp.abuse_incidents < ABUSE_THRESHOLD_FOR_BAN {
        return false;
    }

    let Some(ip) = &fp.last_ip_address else {
        return false;
    };

    let params = BanIpParams::new(
        ip,
        "Automated: repeated behavioral violations",
        BanDuration::Days(7),
        "behavioral_analysis",
    )
    .with_source_fingerprint(&fp.fingerprint_hash);

    let result = repo.ban_ip(params).await;

    log_ban_result(ip, &fp.fingerprint_hash, fp.abuse_incidents, &result)
}

#[allow(clippy::cognitive_complexity)]
fn log_ban_result(
    ip: &str,
    fingerprint: &str,
    abuse_incidents: i32,
    result: &Result<(), anyhow::Error>,
) -> bool {
    match result {
        Ok(()) => {
            warn!(ip = %ip, fingerprint = %fingerprint, abuse_incidents = abuse_incidents, "Banned IP for behavioral violations");
            true
        },
        Err(e) => {
            warn!(ip = %ip, fingerprint = %fingerprint, error = %e, "Failed to ban IP");
            false
        },
    }
}

systemprompt_provider_contracts::submit_job!(&BehavioralAnalysisJob);
