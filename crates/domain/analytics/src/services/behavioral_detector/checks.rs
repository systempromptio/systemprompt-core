use std::collections::HashSet;

use chrono::{DateTime, Utc};

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
                details: "Navigation pattern is sequential/systematic (bot-like)".to_string(),
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

        if let Some(variance) = compute_timing_variance(&input.request_timestamps) {
            if variance < thresholds::TIMING_VARIANCE_MIN {
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
        if let Some(ref ua) = input.user_agent {
            if is_outdated_browser(ua) {
                *score += scoring::OUTDATED_BROWSER;
                signals.push(BehavioralSignal {
                    signal_type: SignalType::OutdatedBrowser,
                    points: scoring::OUTDATED_BROWSER,
                    details: "Browser version is outdated".to_string(),
                });
            }
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

fn is_sequential_crawl(endpoints: &[String]) -> bool {
    if endpoints.len() < 5 {
        return false;
    }

    let mut sorted = endpoints.to_vec();
    sorted.sort();

    let matches = endpoints
        .iter()
        .enumerate()
        .filter(|(i, endpoint)| *i < sorted.len() && *endpoint == &sorted[*i])
        .count();

    (matches as f64 / endpoints.len() as f64) > 0.7
}

fn compute_timing_variance(timestamps: &[DateTime<Utc>]) -> Option<f64> {
    if timestamps.len() < 2 {
        return None;
    }

    let intervals: Vec<f64> = timestamps
        .windows(2)
        .map(|w| (w[1] - w[0]).num_milliseconds() as f64)
        .collect();

    if intervals.is_empty() {
        return None;
    }

    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    if mean <= 0.0 {
        return None;
    }

    let variance =
        intervals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
    let std_dev = variance.sqrt();

    Some(std_dev / mean)
}

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

fn is_outdated_browser(user_agent: &str) -> bool {
    let ua_lower = user_agent.to_lowercase();

    if let Some(pos) = ua_lower.find("chrome/")
        && let Some(dot_pos) = ua_lower[pos + 7..].find('.')
        && let Ok(major) = ua_lower[pos + 7..][..dot_pos].parse::<i32>()
        && major < thresholds::CHROME_MIN_VERSION
    {
        return true;
    }

    if let Some(pos) = ua_lower.find("firefox/") {
        let version_str = &ua_lower[pos + 8..];
        if let Some(space_pos) = version_str.find(|c: char| !c.is_numeric() && c != '.')
            && let Ok(major) = version_str[..space_pos].parse::<i32>()
            && major < thresholds::FIREFOX_MIN_VERSION
        {
            return true;
        } else if let Ok(major) = version_str.parse::<i32>()
            && major < thresholds::FIREFOX_MIN_VERSION
        {
            return true;
        }
    }

    false
}
