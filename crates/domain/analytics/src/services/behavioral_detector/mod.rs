//! Behavioural-bot detector — combines a battery of heuristic checks across
//! a single session and across all sessions sharing a fingerprint to assign
//! a 0-100 suspicion score and a list of triggered [`BehavioralSignal`]s.

mod checks;
mod fingerprint_checks;
mod helpers;
mod types;

pub use types::{BehavioralAnalysisInput, BehavioralAnalysisResult, BehavioralSignal, SignalType};

pub const BEHAVIORAL_BOT_THRESHOLD: i32 = 30;

mod scoring {
    pub(crate) const HIGH_REQUEST_COUNT: i32 = 30;
    pub(crate) const HIGH_PAGE_COVERAGE: i32 = 25;
    pub(crate) const SEQUENTIAL_NAVIGATION: i32 = 20;
    pub(crate) const MULTIPLE_FINGERPRINT_SESSIONS: i32 = 20;
    pub(crate) const REGULAR_TIMING: i32 = 15;
    pub(crate) const HIGH_PAGES_PER_MINUTE: i32 = 15;
    pub(crate) const OUTDATED_BROWSER: i32 = 25;
    pub(crate) const NO_JAVASCRIPT_EVENTS: i32 = 20;
    pub(crate) const GHOST_SESSION: i32 = 35;
    pub(crate) const RESIDENTIAL_PROXY_ROTATION: i32 = 35;
    pub(crate) const NO_ENGAGEMENT_ACROSS_SESSIONS: i32 = 25;
    pub(crate) const PERIODIC_CADENCE: i32 = 35;
    pub(crate) const HOME_TAB_WATCHER: i32 = 35;
}

mod thresholds {
    pub(crate) const REQUEST_COUNT_LIMIT: i64 = 50;
    pub(crate) const PAGE_COVERAGE_PERCENT: f64 = 60.0;
    pub(crate) const FINGERPRINT_SESSION_LIMIT: i64 = 5;
    pub(crate) const PAGES_PER_MINUTE_LIMIT: f64 = 5.0;
    pub(crate) const TIMING_VARIANCE_MIN: f64 = 0.1;
    pub(crate) const CHROME_MIN_VERSION: i32 = 120;
    pub(crate) const FIREFOX_MIN_VERSION: i32 = 120;
    pub(crate) const NO_JS_MIN_REQUESTS: i64 = 2;
    pub(crate) const GHOST_SESSION_MIN_AGE_SECONDS: i64 = 30;
    pub(crate) const RESIDENTIAL_PROXY_IP_RATIO: f64 = 0.8;
    pub(crate) const RESIDENTIAL_PROXY_MIN_SESSIONS: i64 = 5;
    pub(crate) const NO_ENGAGEMENT_MIN_SESSIONS: i64 = 10;
    pub(crate) const PERIODIC_CADENCE_MIN_SESSIONS: usize = 5;
    pub(crate) const PERIODIC_CADENCE_MAX_CV: f64 = 0.1;
    pub(crate) const HOME_TAB_REQUEST_CEILING: i64 = 2;
    pub(crate) const HOME_TAB_DAILY_GAP_SECONDS_MIN: f64 = 60.0 * 60.0 * 20.0;
    pub(crate) const HOME_TAB_DAILY_GAP_SECONDS_MAX: f64 = 60.0 * 60.0 * 28.0;
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
        Self::check_no_javascript_events(input, &mut score, &mut signals);
        Self::check_ghost_session(input, &mut score, &mut signals);
        Self::check_residential_proxy_rotation(input, &mut score, &mut signals);
        Self::check_no_engagement_across_sessions(input, &mut score, &mut signals);
        Self::check_periodic_cadence(input, &mut score, &mut signals);
        Self::check_home_tab_watcher(input, &mut score, &mut signals);

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
}
