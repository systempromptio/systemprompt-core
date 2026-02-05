mod checks;
mod types;

pub use types::{BehavioralAnalysisInput, BehavioralAnalysisResult, BehavioralSignal, SignalType};

pub const BEHAVIORAL_BOT_THRESHOLD: i32 = 30;

pub mod scoring {
    pub const HIGH_REQUEST_COUNT: i32 = 30;
    pub const HIGH_PAGE_COVERAGE: i32 = 25;
    pub const SEQUENTIAL_NAVIGATION: i32 = 20;
    pub const MULTIPLE_FINGERPRINT_SESSIONS: i32 = 20;
    pub const REGULAR_TIMING: i32 = 15;
    pub const HIGH_PAGES_PER_MINUTE: i32 = 15;
    pub const OUTDATED_BROWSER: i32 = 25;
    pub const NO_JAVASCRIPT_EVENTS: i32 = 20;
    pub const GHOST_SESSION: i32 = 35;
}

pub mod thresholds {
    pub const REQUEST_COUNT_LIMIT: i64 = 50;
    pub const PAGE_COVERAGE_PERCENT: f64 = 60.0;
    pub const FINGERPRINT_SESSION_LIMIT: i64 = 5;
    pub const PAGES_PER_MINUTE_LIMIT: f64 = 5.0;
    pub const TIMING_VARIANCE_MIN: f64 = 0.1;
    pub const CHROME_MIN_VERSION: i32 = 120;
    pub const FIREFOX_MIN_VERSION: i32 = 120;
    pub const NO_JS_MIN_REQUESTS: i64 = 3;
    pub const GHOST_SESSION_MIN_AGE_SECONDS: i64 = 30;
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
