//! Single-session behavioural-bot checks. Cross-session/fingerprint checks
//! live in [`super::fingerprint_checks`]; pure helpers live in
//! [`super::helpers`].

use std::collections::HashSet;

use super::helpers::{compute_timing_variance, is_outdated_browser, is_sequential_crawl};
use super::{
    BehavioralAnalysisInput, BehavioralBotDetector, BehavioralSignal, SignalType, scoring,
    thresholds,
};

impl BehavioralBotDetector {
    pub(super) fn check_high_request_count(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.request_count > thresholds::REQUEST_COUNT_LIMIT {
            *score += scoring::HIGH_REQUEST_COUNT;
            signals.push(BehavioralSignal {
                signal_type: SignalType::HighRequestCount,
                points: scoring::HIGH_REQUEST_COUNT,
                details: format!(
                    "Request count {} exceeds threshold {}",
                    input.request_count,
                    thresholds::REQUEST_COUNT_LIMIT
                ),
            });
        }
    }

    pub(super) fn check_high_page_coverage(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.total_site_pages <= 0 {
            return;
        }

        let unique_pages = input
            .endpoints_accessed
            .iter()
            .collect::<HashSet<_>>()
            .len();
        let coverage = (unique_pages as f64 / input.total_site_pages as f64) * 100.0;

        if coverage > thresholds::PAGE_COVERAGE_PERCENT {
            *score += scoring::HIGH_PAGE_COVERAGE;
            signals.push(BehavioralSignal {
                signal_type: SignalType::HighPageCoverage,
                points: scoring::HIGH_PAGE_COVERAGE,
                details: format!(
                    "Page coverage {:.1}% exceeds {}%",
                    coverage,
                    thresholds::PAGE_COVERAGE_PERCENT
                ),
            });
        }
    }

    pub(super) fn check_sequential_navigation(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.endpoints_accessed.len() >= 5 && is_sequential_crawl(&input.endpoints_accessed) {
            *score += scoring::SEQUENTIAL_NAVIGATION;
            signals.push(BehavioralSignal {
                signal_type: SignalType::SequentialNavigation,
                points: scoring::SEQUENTIAL_NAVIGATION,
                details: "Navigation pattern is sequential/systematic (bot-like)".to_owned(),
            });
        }
    }

    pub(super) fn check_multiple_fingerprint_sessions(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.fingerprint_session_count > thresholds::FINGERPRINT_SESSION_LIMIT {
            *score += scoring::MULTIPLE_FINGERPRINT_SESSIONS;
            signals.push(BehavioralSignal {
                signal_type: SignalType::MultipleFingerPrintSessions,
                points: scoring::MULTIPLE_FINGERPRINT_SESSIONS,
                details: format!(
                    "Fingerprint has {} sessions, exceeds {}",
                    input.fingerprint_session_count,
                    thresholds::FINGERPRINT_SESSION_LIMIT
                ),
            });
        }
    }

    pub(super) fn check_regular_timing(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.request_timestamps.len() < 5 {
            return;
        }

        if let Some(variance) = compute_timing_variance(&input.request_timestamps)
            && variance < thresholds::TIMING_VARIANCE_MIN
        {
            *score += scoring::REGULAR_TIMING;
            signals.push(BehavioralSignal {
                signal_type: SignalType::RegularTiming,
                points: scoring::REGULAR_TIMING,
                details: format!(
                    "Request timing variance {:.3} is suspiciously regular",
                    variance
                ),
            });
        }
    }

    pub(super) fn check_high_pages_per_minute(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        let duration_minutes =
            (input.last_activity_at - input.started_at).num_seconds() as f64 / 60.0;

        if duration_minutes <= 0.0 {
            return;
        }

        let pages_per_minute = input.endpoints_accessed.len() as f64 / duration_minutes;
        if pages_per_minute > thresholds::PAGES_PER_MINUTE_LIMIT {
            *score += scoring::HIGH_PAGES_PER_MINUTE;
            signals.push(BehavioralSignal {
                signal_type: SignalType::HighPagesPerMinute,
                points: scoring::HIGH_PAGES_PER_MINUTE,
                details: format!(
                    "Pages/min {:.2} exceeds {}",
                    pages_per_minute,
                    thresholds::PAGES_PER_MINUTE_LIMIT
                ),
            });
        }
    }

    pub(super) fn check_outdated_browser(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if let Some(ref ua) = input.user_agent
            && is_outdated_browser(ua)
        {
            *score += scoring::OUTDATED_BROWSER;
            signals.push(BehavioralSignal {
                signal_type: SignalType::OutdatedBrowser,
                points: scoring::OUTDATED_BROWSER,
                details: "Browser version is outdated".to_owned(),
            });
        }
    }

    pub(super) fn check_no_javascript_events(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.request_count >= thresholds::NO_JS_MIN_REQUESTS && !input.has_javascript_events {
            *score += scoring::NO_JAVASCRIPT_EVENTS;
            signals.push(BehavioralSignal {
                signal_type: SignalType::NoJavaScriptEvents,
                points: scoring::NO_JAVASCRIPT_EVENTS,
                details: format!(
                    "Session has {} requests but no JavaScript analytics events",
                    input.request_count
                ),
            });
        }
    }

    pub(super) fn check_ghost_session(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        let session_age = (input.last_activity_at - input.started_at).num_seconds();

        if input.landing_page.is_none()
            && input.entry_url.is_none()
            && input.request_count == 0
            && session_age >= thresholds::GHOST_SESSION_MIN_AGE_SECONDS
        {
            *score += scoring::GHOST_SESSION;
            signals.push(BehavioralSignal {
                signal_type: SignalType::GhostSession,
                points: scoring::GHOST_SESSION,
                details: format!(
                    "Ghost session: no landing page, no entry URL, 0 requests after {}s",
                    session_age
                ),
            });
        }
    }
}
