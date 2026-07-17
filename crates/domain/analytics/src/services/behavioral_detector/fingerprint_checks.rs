//! Cross-session / fingerprint-windowed behavioural-bot checks. Split from
//! `checks.rs` to keep modules under 300 lines.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::helpers::compute_timing_variance;
use super::{
    BehavioralAnalysisInput, BehavioralBotDetector, BehavioralSignal, SignalType, scoring,
    thresholds,
};

impl BehavioralBotDetector {
    pub(super) fn check_residential_proxy_rotation(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.fingerprint_session_count < thresholds::RESIDENTIAL_PROXY_MIN_SESSIONS {
            return;
        }

        let ratio =
            input.fingerprint_unique_ip_count as f64 / input.fingerprint_session_count as f64;
        if ratio >= thresholds::RESIDENTIAL_PROXY_IP_RATIO {
            *score += scoring::RESIDENTIAL_PROXY_ROTATION;
            signals.push(BehavioralSignal {
                signal_type: SignalType::ResidentialProxyRotation,
                points: scoring::RESIDENTIAL_PROXY_ROTATION,
                details: format!(
                    "{} sessions across {} unique IPs (ratio {:.2})",
                    input.fingerprint_session_count, input.fingerprint_unique_ip_count, ratio
                ),
            });
        }
    }

    pub(super) fn check_no_engagement_across_sessions(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.fingerprint_session_count >= thresholds::NO_ENGAGEMENT_MIN_SESSIONS
            && input.fingerprint_engagement_event_count == 0
        {
            *score += scoring::NO_ENGAGEMENT_ACROSS_SESSIONS;
            signals.push(BehavioralSignal {
                signal_type: SignalType::NoEngagementAcrossSessions,
                points: scoring::NO_ENGAGEMENT_ACROSS_SESSIONS,
                details: format!(
                    "{} sessions for fingerprint with zero engagement events",
                    input.fingerprint_session_count
                ),
            });
        }
    }

    pub(super) fn check_periodic_cadence(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.fingerprint_session_starts.len() < thresholds::PERIODIC_CADENCE_MIN_SESSIONS {
            return;
        }

        let Some(cv) = compute_timing_variance(&input.fingerprint_session_starts) else {
            return;
        };

        if cv < thresholds::PERIODIC_CADENCE_MAX_CV {
            *score += scoring::PERIODIC_CADENCE;
            signals.push(BehavioralSignal {
                signal_type: SignalType::PeriodicCadence,
                points: scoring::PERIODIC_CADENCE,
                details: format!(
                    "Inter-session gap CV {:.4} across {} sessions (cron-like)",
                    cv,
                    input.fingerprint_session_starts.len()
                ),
            });
        }
    }

    pub(super) fn check_home_tab_watcher(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.request_count > thresholds::HOME_TAB_REQUEST_CEILING {
            return;
        }
        if input.fingerprint_session_count < thresholds::RESIDENTIAL_PROXY_MIN_SESSIONS {
            return;
        }
        let unique_endpoints: HashSet<&String> = input.endpoints_accessed.iter().collect();
        if unique_endpoints.len() != 1 {
            return;
        }
        if !input
            .endpoints_accessed
            .first()
            .is_some_and(|e| e == "/" || e.is_empty())
        {
            return;
        }
        if input.fingerprint_session_starts.len() < 2 {
            return;
        }

        let mut intervals_seconds: Vec<f64> = input
            .fingerprint_session_starts
            .windows(2)
            .map(|w| (w[1] - w[0]).num_seconds() as f64)
            .collect();
        if intervals_seconds.is_empty() {
            return;
        }
        intervals_seconds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = intervals_seconds[intervals_seconds.len() / 2];
        if !(thresholds::HOME_TAB_DAILY_GAP_SECONDS_MIN
            ..=thresholds::HOME_TAB_DAILY_GAP_SECONDS_MAX)
            .contains(&median)
        {
            return;
        }

        *score += scoring::HOME_TAB_WATCHER;
        signals.push(BehavioralSignal {
            signal_type: SignalType::HomeTabWatcher,
            points: scoring::HOME_TAB_WATCHER,
            details: format!(
                "Single-endpoint '/' session with ~daily cadence ({} sessions, median gap {:.0}s)",
                input.fingerprint_session_count, median
            ),
        });
    }
}
