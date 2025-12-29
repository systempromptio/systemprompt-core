use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const BEHAVIORAL_BOT_THRESHOLD: i32 = 50;

pub mod scoring {
    pub const HIGH_REQUEST_COUNT: i32 = 30;
    pub const HIGH_PAGE_COVERAGE: i32 = 25;
    pub const SEQUENTIAL_NAVIGATION: i32 = 20;
    pub const MULTIPLE_FINGERPRINT_SESSIONS: i32 = 20;
    pub const REGULAR_TIMING: i32 = 15;
    pub const HIGH_PAGES_PER_MINUTE: i32 = 15;
    pub const OUTDATED_BROWSER: i32 = 10;
}

pub mod thresholds {
    pub const REQUEST_COUNT_LIMIT: i64 = 50;
    pub const PAGE_COVERAGE_PERCENT: f64 = 60.0;
    pub const FINGERPRINT_SESSION_LIMIT: i64 = 5;
    pub const PAGES_PER_MINUTE_LIMIT: f64 = 5.0;
    pub const TIMING_VARIANCE_MIN: f64 = 0.1;
    pub const CHROME_MIN_VERSION: i32 = 90;
    pub const FIREFOX_MIN_VERSION: i32 = 88;
}

#[derive(Debug, Clone)]
pub struct BehavioralAnalysisInput {
    pub session_id: String,
    pub fingerprint_hash: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: i64,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub endpoints_accessed: Vec<String>,
    pub total_site_pages: i64,
    pub fingerprint_session_count: i64,
    pub request_timestamps: Vec<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralAnalysisResult {
    pub score: i32,
    pub is_suspicious: bool,
    pub signals: Vec<BehavioralSignal>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehavioralSignal {
    pub signal_type: SignalType,
    pub points: i32,
    pub details: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalType {
    HighRequestCount,
    HighPageCoverage,
    SequentialNavigation,
    MultipleFingerPrintSessions,
    RegularTiming,
    HighPagesPerMinute,
    OutdatedBrowser,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HighRequestCount => write!(f, "high_request_count"),
            Self::HighPageCoverage => write!(f, "high_page_coverage"),
            Self::SequentialNavigation => write!(f, "sequential_navigation"),
            Self::MultipleFingerPrintSessions => write!(f, "multiple_fingerprint_sessions"),
            Self::RegularTiming => write!(f, "regular_timing"),
            Self::HighPagesPerMinute => write!(f, "high_pages_per_minute"),
            Self::OutdatedBrowser => write!(f, "outdated_browser"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BehavioralBotDetector;

impl BehavioralBotDetector {
    pub const fn new() -> Self {
        Self
    }

    pub fn analyze(input: &BehavioralAnalysisInput) -> BehavioralAnalysisResult {
        let mut signals = Vec::new();
        let mut score = 0;

        Self::check_high_request_count(input, &mut score, &mut signals);
        Self::check_high_page_coverage(input, &mut score, &mut signals);
        Self::check_sequential_navigation(input, &mut score, &mut signals);
        Self::check_multiple_fingerprint_sessions(input, &mut score, &mut signals);
        Self::check_regular_timing(input, &mut score, &mut signals);
        Self::check_high_pages_per_minute(input, &mut score, &mut signals);
        Self::check_outdated_browser(input, &mut score, &mut signals);

        let is_suspicious = score >= BEHAVIORAL_BOT_THRESHOLD;
        let reason = is_suspicious.then(|| {
            signals
                .iter()
                .map(|s| s.signal_type.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        });

        BehavioralAnalysisResult {
            score,
            is_suspicious,
            signals,
            reason,
        }
    }

    fn check_high_request_count(
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

    fn check_high_page_coverage(
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

    fn check_sequential_navigation(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.endpoints_accessed.len() >= 5
            && Self::is_sequential_crawl(&input.endpoints_accessed)
        {
            *score += scoring::SEQUENTIAL_NAVIGATION;
            signals.push(BehavioralSignal {
                signal_type: SignalType::SequentialNavigation,
                points: scoring::SEQUENTIAL_NAVIGATION,
                details: "Navigation pattern is sequential/systematic (bot-like)".to_string(),
            });
        }
    }

    fn check_multiple_fingerprint_sessions(
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

    fn check_regular_timing(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if input.request_timestamps.len() < 5 {
            return;
        }

        if let Some(variance) = Self::compute_timing_variance(&input.request_timestamps) {
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

    fn check_high_pages_per_minute(
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

    fn check_outdated_browser(
        input: &BehavioralAnalysisInput,
        score: &mut i32,
        signals: &mut Vec<BehavioralSignal>,
    ) {
        if let Some(ref ua) = input.user_agent {
            if Self::is_outdated_browser(ua) {
                *score += scoring::OUTDATED_BROWSER;
                signals.push(BehavioralSignal {
                    signal_type: SignalType::OutdatedBrowser,
                    points: scoring::OUTDATED_BROWSER,
                    details: "Browser version is outdated".to_string(),
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

    fn is_outdated_browser(user_agent: &str) -> bool {
        let ua_lower = user_agent.to_lowercase();

        if let Some(pos) = ua_lower.find("chrome/") {
            let version_str = &ua_lower[pos + 7..];
            if let Some(dot_pos) = version_str.find('.') {
                if let Ok(major) = version_str[..dot_pos].parse::<i32>() {
                    if major < thresholds::CHROME_MIN_VERSION {
                        return true;
                    }
                }
            }
        }

        if let Some(pos) = ua_lower.find("firefox/") {
            let version_str = &ua_lower[pos + 8..];
            if let Some(space_pos) = version_str.find(|c: char| !c.is_numeric() && c != '.') {
                if let Ok(major) = version_str[..space_pos].parse::<i32>() {
                    if major < thresholds::FIREFOX_MIN_VERSION {
                        return true;
                    }
                }
            } else if let Ok(major) = version_str.parse::<i32>() {
                if major < thresholds::FIREFOX_MIN_VERSION {
                    return true;
                }
            }
        }

        false
    }
}
