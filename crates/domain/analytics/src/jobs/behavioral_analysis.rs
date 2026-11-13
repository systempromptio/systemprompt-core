//! Behavioural-analysis job: scans fingerprint reputation rows and flags
//! suspicious patterns. IPs past the abuse threshold are banned only when the
//! job's `enforce` config flag is set; otherwise qualifying IPs are logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_traits::{
    DynIpBanStore, IpBanRequest, IpBanStoreError, Job, JobContext, JobResult, ProviderError,
    ProviderResult,
};
use tracing::{info, warn};

use crate::repository::AnalyticsRepositories;
use crate::{
    ABUSE_THRESHOLD_FOR_BAN, FingerprintRepository, FingerprintReputation, FlagReason,
    HIGH_REQUEST_THRESHOLD, HIGH_VELOCITY_RPM, SUSTAINED_VELOCITY_MINUTES,
};

const BAN_DURATION_DAYS: i64 = 7;

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

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(ctx.get::<DbPool>()?);

        let sessions = Arc::clone(&ctx.get::<Arc<AnalyticsRepositories>>()?.session_store);
        let fingerprint_repo = FingerprintRepository::new(&db_pool, sessions);
        let banned_ip_repo = Arc::clone(ctx.get::<DynIpBanStore>()?);

        info!("Starting behavioral analysis job");

        let fingerprints = fingerprint_repo
            .get_fingerprints_for_analysis()
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        let stats = process_fingerprints(
            &fingerprints,
            &fingerprint_repo,
            banned_ip_repo.as_ref(),
            ctx.enforce(),
        )
        .await;
        let expired_cleaned = match banned_ip_repo.cleanup_expired().await {
            Ok(count) => count,
            Err(e) => {
                warn!(error = %e, "Failed to cleanup expired bans");
                0
            },
        };

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
    banned_ip_repo: &dyn systemprompt_traits::IpBanStore,
    enforce: bool,
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

        if ban_ip_if_needed(fp, banned_ip_repo, enforce).await {
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

fn log_flag_result(
    fingerprint: &str,
    reasons: &[FlagReason],
    new_score: i32,
    result: &crate::Result<()>,
) -> bool {
    match result {
        Ok(()) => {
            warn!(
                fingerprint = %fingerprint,
                reasons = ?reasons,
                new_score = new_score,
                "Flagged fingerprint"
            );
            true
        },
        Err(e) => {
            warn!(
                fingerprint = %fingerprint,
                error = %e,
                "Failed to flag fingerprint"
            );
            false
        },
    }
}

async fn ban_ip_if_needed(
    fp: &FingerprintReputation,
    repo: &dyn systemprompt_traits::IpBanStore,
    enforce: bool,
) -> bool {
    if fp.abuse_incidents < ABUSE_THRESHOLD_FOR_BAN {
        return false;
    }

    let Some(ip) = &fp.last_ip_address else {
        return false;
    };

    if !enforce {
        info!(
            ip = %ip,
            fingerprint = %fp.fingerprint_hash,
            abuse_incidents = fp.abuse_incidents,
            "enforce disabled: IP qualifies for a behavioral ban but was not banned"
        );
        return false;
    }

    let result = repo
        .ban_ip(IpBanRequest {
            ip_address: ip,
            reason: "Automated: repeated behavioral violations",
            duration_days: BAN_DURATION_DAYS,
            source_fingerprint: Some(&fp.fingerprint_hash),
            ban_source: "behavioral_analysis",
        })
        .await;

    log_ban_result(ip, &fp.fingerprint_hash, fp.abuse_incidents, &result)
}

fn log_ban_result(
    ip: &str,
    fingerprint: &str,
    abuse_incidents: i32,
    result: &Result<(), IpBanStoreError>,
) -> bool {
    match result {
        Ok(()) => {
            warn!(
                ip = %ip,
                fingerprint = %fingerprint,
                abuse_incidents = abuse_incidents,
                "Banned IP for behavioral violations"
            );
            true
        },
        Err(e) => {
            warn!(
                ip = %ip,
                fingerprint = %fingerprint,
                error = %e,
                "Failed to ban IP"
            );
            false
        },
    }
}

systemprompt_traits::submit_job!(&BehavioralAnalysisJob);
